#!/usr/bin/env python3
"""Run or verify typed action result-request/goal-response native reuse evidence."""

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
BOUNDARY = "typed action result-request preparation plus goal-response fill/decode; no RCL/DDS"


def sha256(data):
    return hashlib.sha256(data).hexdigest()


def validate(report, root=None):
    metadata = report["metadata"]
    records = report["records"]
    if (metadata.get("boundary") != BOUNDARY
            or metadata.get("timing_qualified") is not False
            or metadata.get("iterations") != 10_000
            or len(records) != metadata["repetitions"] * 2):
        raise RuntimeError("invalid typed action lifecycle reuse metadata")
    for repetition in range(metadata["repetitions"]):
        pair = [record for record in records if record["repetition"] == repetition]
        if {record["native_storage"] for record in pair} != {"fresh_native", "reused_native"}:
            raise RuntimeError("incomplete typed action lifecycle repetition")
        fresh = next(record for record in pair if record["native_storage"] == "fresh_native")
        reused = next(record for record in pair if record["native_storage"] == "reused_native")
        for record in pair:
            if (record["iterations"] != metadata["iterations"]
                    or record["boundary"] != BOUNDARY
                    or type(record["elapsed_ns"]) is not int
                    or record["elapsed_ns"] <= 0
                    or any(type(record[field]) is not int or record[field] < 0
                           for field in ["allocations", "frees", "requested_bytes"])):
                raise RuntimeError("invalid typed action lifecycle record")
        if (fresh["allocations"] - reused["allocations"] < metadata["iterations"] * 2
                or fresh["frees"] - reused["frees"] < metadata["iterations"] * 2
                or fresh["requested_bytes"] - reused["requested_bytes"]
                < metadata["iterations"] * 28
                or reused["allocations"] != 0
                or reused["frees"] != 0
                or reused["requested_bytes"] != 0):
            raise RuntimeError("typed action lifecycle allocation gate failed")
    for storage in ["fresh_native", "reused_native"]:
        group = [record for record in records if record["native_storage"] == storage]
        for field in ["allocations", "frees", "requested_bytes", "elapsed_ns"]:
            if report["summary"][storage][field] != statistics.median(
                    record[field] for record in group):
                raise RuntimeError("typed action lifecycle summary mismatch")
    compressed = (Path(metadata["report_path"]).parent / metadata["raw_log"]["file"]).read_bytes()
    if sha256(compressed) != metadata["raw_log"]["sha256"]:
        raise RuntimeError("typed action lifecycle raw log mismatch")
    if root is not None:
        for name, expected in metadata["source_sha256"].items():
            path = root / name
            if not path.is_file() or sha256(path.read_bytes()) != expected:
                raise RuntimeError(f"typed action lifecycle source mismatch: {name}")


def run(args):
    limit = 10 * 1024**3
    soft, hard = resource.getrlimit(resource.RLIMIT_AS)
    resource.setrlimit(resource.RLIMIT_AS, (min(soft, limit) if soft > 0 else limit, hard))
    probe = args.probe.resolve()
    env = os.environ.copy()
    env.update(
        CARGO_BUILD_JOBS="1", CARGO_NET_OFFLINE="true", RAYON_NUM_THREADS="1",
        RUST_TEST_THREADS="1", LD_PRELOAD=str(probe)
        + ((":" + env["LD_PRELOAD"]) if env.get("LD_PRELOAD") else ""),
    )
    command = [
        "taskset", "-c", str(args.cpu), "cargo", "test", "--offline", "--locked", "--release",
        "-p", "r2r", "--lib",
        "benchmark_typed_action_lifecycle_fresh_vs_reused_native_storage",
        "--", "--ignored", "--nocapture", "--test-threads=1",
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
            raise RuntimeError("typed action lifecycle stage did not emit two records")
        for record in parsed:
            record["repetition"] = repetition
            records.append(record)
    compressed = gzip.compress("\n".join(raw).encode(), mtime=0)
    args.output.parent.mkdir(parents=True, exist_ok=True)
    raw_path = args.output.with_suffix(".log.gz")
    raw_path.write_bytes(compressed)
    sources = [
        Path("r2r/src/action_clients.rs"),
        Path("r2r/src/action_clients/goal_request_buffer_tests.rs"),
        Path("r2r/src/nodes/spin/dispatch/actions.rs"),
        Path("r2r/src/msg_types.rs"),
        Path("r2r/src/native_buffer.rs"),
        Path("r2r/src/nodes/action_client_api.rs"),
        Path("r2r/benches/support/allocation_probe.rs"),
        Path("tools/bench_ros/typed_action_lifecycle_native_reuse_stage.py"),
        Path("tools/bench_ros/probe/probe.c"),
        Path("tools/bench_ros/probe/probe.h"),
    ]
    report = {
        "metadata": {
            "boundary": BOUNDARY, "timing_qualified": False,
            "iterations": 10_000, "repetitions": args.repetitions, "cpu": args.cpu,
            "single_cpu_affinity": True, "virtual_memory_limit_bytes": limit,
            "probe_sha256": sha256(probe.read_bytes()),
            "source_sha256": {str(path): sha256((ROOT / path).read_bytes()) for path in sources},
            "raw_log": {"file": raw_path.name, "sha256": sha256(compressed)},
            "report_path": str(args.output.resolve()),
        },
        "records": records,
        "summary": {
            storage: {
                field: statistics.median(
                    record[field] for record in records if record["native_storage"] == storage)
                for field in ["allocations", "frees", "requested_bytes", "elapsed_ns"]
            }
            for storage in ["fresh_native", "reused_native"]
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
            parser.error("invalid typed action lifecycle stage configuration")
        run(args)
    else:
        report = json.loads(args.report.read_text())
        report["metadata"]["report_path"] = str(args.report.resolve())
        validate(report, ROOT if args.require_current_sources else None)
        print(f"{args.report}: typed action lifecycle native reuse stage verified")


if __name__ == "__main__":
    main()
