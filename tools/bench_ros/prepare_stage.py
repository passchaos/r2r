#!/usr/bin/env python3
"""Build, measure and verify native parked-server preparation stages."""

import argparse
import gzip
import hashlib
import io
import json
import math
import os
from pathlib import Path
import platform
import shutil
import statistics
import subprocess
import tarfile
import tempfile

ROOT = Path(__file__).resolve().parents[2]
HARNESS = Path("r2r/src/nodes/action_server_scheduling_tests.rs")
TEST = "nodes::action_server_scheduling_tests::benchmark_native_parked_server_prepare"
BOUNDARY = "Node::prepare_spin only; optimized lib test profile with TLS poll instrumentation"
FIELDS = ("p50_ns", "p95_ns", "p99_ns", "ns_per_call")


def digest(path):
    return hashlib.sha256(path.read_bytes()).hexdigest()


def source_identity(source):
    paths = [source / "Cargo.lock", source / "Cargo.toml", source / "rust-toolchain.toml"]
    for package in ["r2r", "r2r_actions", "r2r_rcl", "r2r_common", "r2r_msg_gen", "r2r_macros"]:
        paths.append(source / package / "Cargo.toml")
        paths.extend(sorted((source / package / "src").rglob("*.rs")))
    return {str(p.relative_to(source)): digest(p) for p in paths if p.is_file()}


def build_test(source, destination, target, log):
    env = os.environ.copy()
    env["CARGO_TARGET_DIR"] = str(target)
    before = source_identity(source)
    result = subprocess.run(["cargo", "test", "--locked", "--release", "-p", "r2r", "--lib", "--no-run", "--message-format=json"], cwd=source, env=env, stdout=subprocess.PIPE, stderr=log, text=True, check=True)
    artifacts = [json.loads(line) for line in result.stdout.splitlines() if line.startswith("{")]
    binary = next(item["executable"] for item in artifacts if item.get("reason") == "compiler-artifact" and item.get("target", {}).get("name") == "r2r" and item.get("executable"))
    assert before == source_identity(source), "source changed during preparation-stage build"
    shutil.copy2(binary, destination)
    return before


def build(args):
    out = args.output_dir.resolve()
    out.mkdir(parents=True, exist_ok=True)
    archive = subprocess.check_output(["git", "archive", args.baseline_ref], cwd=ROOT)
    with tempfile.TemporaryDirectory(prefix="prepare-baseline-", dir=out) as directory:
        source = Path(directory)
        with tarfile.open(fileobj=io.BytesIO(archive)) as content:
            content.extractall(source, filter="data")
        shutil.copy2(ROOT / "Cargo.lock", source / "Cargo.lock")
        shutil.copy2(ROOT / HARNESS, source / HARNESS)
        node = source / "r2r/src/nodes.rs"
        text = node.read_text()
        if "mod action_server_scheduling_tests;" not in text:
            text += "\n#[cfg(test)]\nmod action_server_scheduling_tests;\n"
            node.write_text(text)
        with (out / "build.log").open("w") as log:
            baseline = build_test(source, out / "baseline", out / "baseline-target", log)
            candidate = build_test(ROOT, out / "candidate", ROOT / "target", log)
    manifest = {"baseline_ref": subprocess.check_output(["git", "rev-parse", args.baseline_ref], cwd=ROOT, text=True).strip(), "harness_sha256": digest(ROOT / HARNESS), "baseline": baseline, "candidate": candidate, "binary_sha256": {label: digest(out / label) for label in ["baseline", "candidate"]}, "boundary": BOUNDARY, "profile": "release lib test", "rustc": subprocess.check_output(["rustc", "--version"], text=True).strip()}
    (out / "build-manifest.json").write_text(json.dumps(manifest, indent=2) + "\n")
    print("Independent preparation-stage binaries:", out, flush=True)


def percentile(samples, p):
    ordered = sorted(samples)
    return ordered[max(0, (len(ordered) * p + 99) // 100 - 1)]


def summarize(records):
    rows = []
    for servers in sorted({r["servers"] for r in records}):
        for label in ["baseline", "candidate"]:
            group = [r for r in records if r["servers"] == servers and r["label"] == label]
            rows.append({"servers": servers, "label": label, **{field: statistics.median(r[field] for r in group) for field in FIELDS}})
    return rows


def regression(summary, maximum):
    assert math.isfinite(maximum) and maximum >= 1
    failures = []
    for servers in sorted({r["servers"] for r in summary}):
        rows = {r["label"]: r for r in summary if r["servers"] == servers}
        for field in FIELDS:
            ratio = rows["candidate"][field] / rows["baseline"][field]
            if ratio > maximum:
                failures.append({"servers": servers, "field": field, "ratio": ratio})
    return {"maximum_ratio": maximum, "passed": not failures, "failures": failures}


def run(args):
    folder = args.binaries.resolve()
    manifest = json.loads((folder / "build-manifest.json").read_text())
    for label, h in manifest["binary_sha256"].items():
        assert digest(folder / label) == h
    env = os.environ.copy()
    env.pop("CYCLONEDDS_URI", None)
    env.update(RMW_IMPLEMENTATION=args.rmw, ROS_DOMAIN_ID=str(args.domain_id), ROS_LOCALHOST_ONLY="1", R2R_PREPARE_ITERATIONS=str(args.iterations), R2R_PREPARE_WARMUP=str(args.warmup))
    records, samples = [], []
    output = args.output.resolve()
    output.parent.mkdir(parents=True, exist_ok=True)
    for servers in args.servers:
        env["R2R_PREPARE_SERVERS"] = str(servers)
        for repetition in range(args.repetitions):
            labels = ["baseline", "candidate"] if repetition % 2 == 0 else ["candidate", "baseline"]
            for label in labels:
                command = ["taskset", "-c", str(args.cpu), str(folder / label), TEST, "--exact", "--ignored", "--nocapture", "--test-threads=1"]
                try:
                    result = subprocess.run(command, cwd=ROOT, env=env, capture_output=True, text=True, timeout=120)
                except subprocess.TimeoutExpired as error:
                    failure = output.with_suffix(f".{servers}-{repetition}-{label}.timeout.log")
                    failure.write_bytes((error.stdout or b"") + (error.stderr or b"") + b"\nnative stage exceeded 120 seconds\n")
                    raise RuntimeError(f"native stage timed out; preserved output: {failure}") from error
                if result.returncode:
                    failure = output.with_suffix(f".{servers}-{repetition}-{label}.failure.log")
                    failure.write_text(result.stdout + result.stderr)
                    raise RuntimeError(f"native stage failed; preserved output: {failure}")
                line = next(line for line in result.stdout.splitlines() if "R2R_PREPARE_STAGE " in line)
                record = json.loads(line.split("R2R_PREPARE_STAGE ", 1)[1])
                raw = record.pop("samples_ns")
                record.update(label=label, repetition=repetition, ns_per_call=record["loop_ns"] / record["iterations"])
                records.append(record)
                samples.append({"servers": servers, "label": label, "repetition": repetition, "samples_ns": raw})
                print(f"{label} servers={servers} repetition={repetition + 1}: p50={record['p50_ns']}ns polls={record['maintenance_polls']}", flush=True)
    sample_path = output.with_suffix(".samples.json.gz")
    sample_path.write_bytes(gzip.compress(json.dumps(samples, separators=(",", ":")).encode(), mtime=0))
    summary = summarize(records)
    report = {"metadata": {"boundary": BOUNDARY, "cpu": args.cpu, "rmw": args.rmw, "domain_id": args.domain_id, "servers": args.servers, "iterations": args.iterations, "warmup": args.warmup, "repetitions": args.repetitions, "platform": platform.platform(), "build_manifest": manifest, "samples": {"file": sample_path.name, "sha256": digest(sample_path)}}, "records": records, "summary": summary, "gate": regression(summary, args.maximum_ratio)}
    output.write_text(json.dumps(report, indent=2) + "\n")
    verify(output, require_pass=False)
    if not report["gate"]["passed"]:
        raise RuntimeError(f"preparation-stage regression: {report['gate']['failures']}")


def verify(path, require_pass=True):
    report = json.loads(path.read_text())
    m = report["metadata"]
    assert m["boundary"] == BOUNDARY
    assert m["servers"] and len(set(m["servers"])) == len(m["servers"])
    assert all(type(value) is int and value >= 0 for value in m["servers"])
    assert type(m["iterations"]) is int and m["iterations"] > 0
    assert type(m["repetitions"]) is int and m["repetitions"] > 0
    assert type(m["warmup"]) is int and m["warmup"] >= 0
    sample_path = path.parent / m["samples"]["file"]
    assert digest(sample_path) == m["samples"]["sha256"]
    samples = json.loads(gzip.decompress(sample_path.read_bytes()))
    key = lambda r: (r["servers"], r["label"], r["repetition"])
    expected = {(servers, label, rep) for servers in m["servers"] for label in ["baseline", "candidate"] for rep in range(m["repetitions"])}
    assert len(report["records"]) == len(samples) == len(expected)
    assert {key(r) for r in report["records"]} == {key(r) for r in samples} == expected
    indexed = {key(r): r["samples_ns"] for r in samples}
    for r in report["records"]:
        raw = indexed[key(r)]
        assert r["iterations"] == len(raw) == m["iterations"] and r["warmup"] == m["warmup"]
        assert r["boundary"] == BOUNDARY and all(type(v) is int and v > 0 for v in raw)
        assert r["rmw_identifier"] == m["rmw"]
        assert r["cpu_affinity"] == str(m["cpu"])
        assert type(r["loop_ns"]) is int and r["loop_ns"] > 0
        assert math.isfinite(r["ns_per_call"])
        for field, p in [("p50_ns", 50), ("p95_ns", 95), ("p99_ns", 99)]:
            assert r[field] == percentile(raw, p)
        assert r["loop_ns"] >= sum(raw) and r["ns_per_call"] == r["loop_ns"] / len(raw)
        expected_polls = r["servers"] * len(raw) if r["label"] == "baseline" else 0
        assert r["maintenance_polls"] == expected_polls
    summary = summarize(report["records"])
    assert report["summary"] == summary
    gate = regression(summary, report["gate"]["maximum_ratio"])
    assert report["gate"] == gate
    if require_pass:
        assert gate["passed"], gate["failures"]
    print(f"{path}: {len(samples)} native stage records and {sum(len(r['samples_ns']) for r in samples)} samples verified", flush=True)


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    commands = parser.add_subparsers(dest="command", required=True)
    p = commands.add_parser("build")
    p.add_argument("--baseline-ref", required=True)
    p.add_argument("--output-dir", type=Path, required=True)
    p = commands.add_parser("run")
    p.add_argument("--binaries", type=Path, required=True)
    p.add_argument("--output", type=Path, required=True)
    p.add_argument("--rmw", required=True)
    p.add_argument("--cpu", type=int, default=24)
    p.add_argument("--domain-id", type=int, default=132)
    p.add_argument("--servers", type=lambda value: [int(x) for x in value.split(",")], default=[0, 1, 8, 32])
    p.add_argument("--iterations", type=int, default=20000)
    p.add_argument("--warmup", type=int, default=1000)
    p.add_argument("--repetitions", type=int, default=5)
    p.add_argument("--maximum-ratio", type=float, default=1.05)
    p = commands.add_parser("verify")
    p.add_argument("reports", nargs="+", type=Path)
    p.add_argument("--allow-performance-failure", action="store_true")
    args = parser.parse_args()
    if args.command == "build":
        build(args)
    elif args.command == "run":
        if args.iterations <= 0 or args.warmup < 0 or args.repetitions < 1 or not args.servers or len(set(args.servers)) != len(args.servers) or any(x < 0 for x in args.servers):
            parser.error("native stage counts must be nonnegative, nonempty and unique")
        run(args)
    else:
        for path in args.reports:
            verify(path, not args.allow_performance_failure)


if __name__ == "__main__":
    main()
