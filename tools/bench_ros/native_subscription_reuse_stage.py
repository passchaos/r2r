#!/usr/bin/env python3
"""Run or verify the deterministic reusable native-subscription allocation stage."""

import argparse
import gzip
import hashlib
import json
import os
from pathlib import Path
import resource
import statistics
import subprocess


ROOT = Path(__file__).resolve().parents[2]
BOUNDARY = "generated native message fill, channel send/receive and payload validation; no RCL/DDS"


def sha256(data):
    return hashlib.sha256(data).hexdigest()


def validate(report, root=None):
    metadata = report["metadata"]
    records = report["records"]
    if (metadata.get("boundary") != BOUNDARY or metadata.get("timing_qualified") is not False
            or metadata.get("iterations") != 10_000 or metadata.get("elements") != 4096
            or len(records) != metadata["repetitions"] * 2):
        raise RuntimeError("invalid reusable subscription stage metadata")
    for repetition in range(metadata["repetitions"]):
        pair = [record for record in records if record["repetition"] == repetition]
        if {record["delivery"] for record in pair} != {"fresh", "reusable"}:
            raise RuntimeError("incomplete reusable subscription stage repetition")
        fresh = next(record for record in pair if record["delivery"] == "fresh")
        reusable = next(record for record in pair if record["delivery"] == "reusable")
        for record in pair:
            if (record["iterations"] != metadata["iterations"]
                    or record["elements"] != metadata["elements"]
                    or record["boundary"] != BOUNDARY
                    or any(type(record[field]) is not int or record[field] <= 0
                           for field in ["allocations", "frees", "requested_bytes", "elapsed_ns"])):
                raise RuntimeError("invalid reusable subscription stage record")
        if (fresh["allocations"] - reusable["allocations"] < 2 * metadata["iterations"]
                or fresh["frees"] - reusable["frees"] < 2 * metadata["iterations"]
                or fresh["requested_bytes"] - reusable["requested_bytes"]
                < metadata["iterations"] * (metadata["elements"] * 8 - 256)):
            raise RuntimeError("reusable native subscription allocation gate failed")
    for delivery in ["fresh", "reusable"]:
        group = [record for record in records if record["delivery"] == delivery]
        for field in ["allocations", "frees", "requested_bytes", "elapsed_ns"]:
            if report["summary"][delivery][field] != statistics.median(
                    record[field] for record in group):
                raise RuntimeError("reusable subscription summary mismatch")
    compressed = (Path(metadata["report_path"]).parent / metadata["raw_log"]["file"]).read_bytes()
    if sha256(compressed) != metadata["raw_log"]["sha256"]:
        raise RuntimeError("reusable subscription raw log mismatch")
    if root is not None:
        for name, expected in metadata["source_sha256"].items():
            path = root / name
            if not path.is_file() or sha256(path.read_bytes()) != expected:
                raise RuntimeError(f"reusable subscription source mismatch: {name}")


def run(args):
    limit = 10 * 1024**3
    soft, hard = resource.getrlimit(resource.RLIMIT_AS)
    resource.setrlimit(resource.RLIMIT_AS, (min(soft, limit) if soft > 0 else limit, hard))
    env = os.environ.copy()
    env.update(
        CARGO_BUILD_JOBS="1",
        CARGO_NET_OFFLINE="true",
        RAYON_NUM_THREADS="1",
        RUST_TEST_THREADS="1",
        LD_PRELOAD=str(args.probe.resolve())
        + ((":" + env["LD_PRELOAD"]) if env.get("LD_PRELOAD") else ""),
    )
    command = [
        "taskset", "-c", str(args.cpu), "cargo", "test", "--offline", "--locked", "--release",
        "-p", "r2r", "--lib", "benchmark_fresh_vs_reusable_native_subscription_delivery", "--",
        "--ignored", "--nocapture", "--test-threads=1",
    ]
    records, raw = [], []
    for repetition in range(args.repetitions):
        completed = subprocess.run(command, cwd=ROOT, env=env, text=True, capture_output=True)
        raw.append(completed.stdout + completed.stderr)
        if completed.returncode:
            raise RuntimeError(raw[-1])
        parsed = []
        for line in raw[-1].splitlines():
            begin = line.find('{"allocations"')
            if begin >= 0:
                parsed.append(json.loads(line[begin:]))
        if len(parsed) != 2:
            raise RuntimeError("allocation stage did not emit exactly two records")
        for record in parsed:
            record["repetition"] = repetition
            records.append(record)
    compressed = gzip.compress("\n".join(raw).encode(), mtime=0)
    args.output.parent.mkdir(parents=True, exist_ok=True)
    raw_path = args.output.with_suffix(".log.gz")
    raw_path.write_bytes(compressed)
    sources = [
        Path("r2r/src/native_buffer.rs"),
        Path("r2r/src/subscribers.rs"),
        Path("r2r/src/subscribers/reusable_native_tests.rs"),
        Path("r2r/benches/support/allocation_probe.rs"),
        Path("tools/bench_ros/native_subscription_reuse_stage.py"),
        Path("tools/bench_ros/probe/probe.c"),
        Path("tools/bench_ros/probe/probe.h"),
    ]
    report = {
        "metadata": {
            "boundary": BOUNDARY,
            "timing_qualified": False,
            "iterations": 10_000,
            "elements": 4096,
            "repetitions": args.repetitions,
            "cpu": args.cpu,
            "single_cpu_affinity": True,
            "virtual_memory_limit_bytes": limit,
            "probe_sha256": sha256(args.probe.read_bytes()),
            "source_sha256": {str(path): sha256((ROOT / path).read_bytes()) for path in sources},
            "raw_log": {"file": raw_path.name, "sha256": sha256(compressed)},
            "report_path": str(args.output.resolve()),
        },
        "records": records,
        "summary": {
            delivery: {
                field: statistics.median(
                    record[field] for record in records if record["delivery"] == delivery)
                for field in ["allocations", "frees", "requested_bytes", "elapsed_ns"]
            }
            for delivery in ["fresh", "reusable"]
        },
    }
    validate(report, ROOT)
    report["metadata"].pop("report_path")
    args.output.write_text(json.dumps(report, indent=2) + "\n")
    print(json.dumps(report["summary"], indent=2))


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    subparsers = parser.add_subparsers(dest="command", required=True)
    runner = subparsers.add_parser("run")
    runner.add_argument("--probe", type=Path, required=True)
    runner.add_argument("--cpu", type=int, required=True)
    runner.add_argument("--repetitions", type=int, default=5)
    runner.add_argument("--output", type=Path, required=True)
    verifier = subparsers.add_parser("verify")
    verifier.add_argument("report", type=Path)
    verifier.add_argument("--require-current-sources", action="store_true")
    args = parser.parse_args()
    if args.command == "run":
        if args.repetitions <= 0 or args.cpu not in os.sched_getaffinity(0):
            parser.error("invalid reusable subscription stage configuration")
        run(args)
    else:
        report = json.loads(args.report.read_text())
        report["metadata"]["report_path"] = str(args.report.resolve())
        validate(report, ROOT if args.require_current_sources else None)
        print(f"{args.report}: reusable native subscription allocation stage verified")


if __name__ == "__main__":
    main()
