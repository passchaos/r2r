#!/usr/bin/env python3
"""Run or verify endpoint construction/destruction resource evidence."""

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
CASES = [
    ("subscriptions", 1),
    ("subscriptions", 8),
    ("subscriptions", 32),
    ("subscriptions", 128),
    ("services", 1),
    ("services", 8),
    ("services", 32),
    ("clients", 1),
    ("clients", 8),
    ("clients", 32),
    ("timers", 1),
    ("timers", 8),
    ("timers", 32),
    ("mixed", 8),
    ("mixed", 32),
]
MAX_RSS_KIB = 262_144
MAX_RSS_SPREAD_KIB = 16_384


def sha256(data):
    return hashlib.sha256(data).hexdigest()


def case_key(record):
    return (record.get("case"), record.get("entities"), record.get("phase"))


def validate(report, root=None):
    metadata = report["metadata"]
    records = report["records"]
    expected = {(case, count, phase) for case, count in CASES for phase in ("create", "destroy")}
    if (len(records) != metadata["repetitions"] * len(expected)
            or metadata.get("timing_qualified") is not False):
        raise RuntimeError("invalid endpoint construction metadata")
    for repetition in range(metadata["repetitions"]):
        group = [record for record in records if record["repetition"] == repetition]
        if {case_key(record) for record in group} != expected:
            raise RuntimeError("incomplete endpoint construction case set")
    for record in records:
        entities = record["entities"]
        if (record["allocations"] < 0
                or record["frees"] < 0
                or record["requested_bytes"] < 0
                or record["process_max_rss_kib"] > MAX_RSS_KIB
                or record["user_cpu_ns"] < 0
                or record["system_cpu_ns"] < 0):
            raise RuntimeError("endpoint construction resource gate failed")
        endpoints = entities * (4 if record["case"] == "mixed" else 1)
        if record["allocations"] > endpoints * 2000 + 5000:
            raise RuntimeError("endpoint construction allocation bound failed")
    for repetition in range(metadata["repetitions"]):
        group = [record for record in records if record["repetition"] == repetition]
        rss = [record["process_max_rss_kib"] for record in group]
        if max(rss) - min(rss) > MAX_RSS_SPREAD_KIB:
            raise RuntimeError("endpoint construction RSS grew across cases")
    for case, count in CASES:
        for phase in ("create", "destroy"):
            key = f"{case}_{count}_{phase}"
            group = [record for record in records if case_key(record) == (case, count, phase)]
            summary = report["summary"][key]
            for field in ["allocations", "frees", "requested_bytes", "user_cpu_ns", "process_max_rss_kib"]:
                if summary[field] != statistics.median(record[field] for record in group):
                    raise RuntimeError("endpoint construction summary mismatch")
    compressed = (Path(metadata["report_path"]).parent / metadata["raw_log"]["file"]).read_bytes()
    if sha256(compressed) != metadata["raw_log"]["sha256"]:
        raise RuntimeError("endpoint construction raw log mismatch")
    if root is not None:
        for name, expected_digest in metadata["source_sha256"].items():
            path = root / name
            if not path.is_file() or sha256(path.read_bytes()) != expected_digest:
                raise RuntimeError(f"endpoint construction source mismatch: {name}")


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
        "-p", "r2r", "--lib", "benchmark_endpoint_construction_resource_scaling",
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
        if len(parsed) != len(CASES) * 2:
            raise RuntimeError("endpoint construction stage did not emit every phase")
        for record in parsed:
            record["repetition"] = repetition
            records.append(record)
    compressed = gzip.compress("\n".join(raw).encode(), mtime=0)
    args.output.parent.mkdir(parents=True, exist_ok=True)
    raw_path = args.output.with_suffix(".log.gz")
    raw_path.write_bytes(compressed)
    sources = [
        Path("r2r/src/nodes.rs"),
        Path("r2r/src/nodes/idle_spin_resource_tests.rs"),
        Path("r2r/src/nodes/subscription_api.rs"),
        Path("r2r/src/nodes/service_api.rs"),
        Path("r2r/src/nodes/client_api.rs"),
        Path("r2r/src/nodes/timers.rs"),
        Path("r2r/src/subscribers.rs"),
        Path("r2r/src/services.rs"),
        Path("r2r/src/clients.rs"),
        Path("r2r/src/native_buffer.rs"),
        Path("r2r/benches/support/allocation_probe.rs"),
        Path("tools/bench_ros/endpoint_construction_resource_stage.py"),
        Path("tools/bench_ros/probe/probe.c"),
        Path("tools/bench_ros/probe/probe.h"),
    ]
    summary = {}
    for case, count in CASES:
        for phase in ("create", "destroy"):
            key = f"{case}_{count}_{phase}"
            group = [record for record in records if case_key(record) == (case, count, phase)]
            summary[key] = {
                field: statistics.median(record[field] for record in group)
                for field in [
                    "allocations", "frees", "requested_bytes", "user_cpu_ns",
                    "process_max_rss_kib",
                ]
            }
    report = {
        "metadata": {
            "timing_qualified": False, "repetitions": args.repetitions, "cpu": args.cpu,
            "single_cpu_affinity": True, "virtual_memory_limit_bytes": limit,
            "rmw": env.get("RMW_IMPLEMENTATION", "rmw_cyclonedds_cpp"),
            "probe_sha256": sha256(probe.read_bytes()),
            "source_sha256": {str(path): sha256((ROOT / path).read_bytes()) for path in sources},
            "raw_log": {"file": raw_path.name, "sha256": sha256(compressed)},
            "report_path": str(args.output.resolve()),
        },
        "records": records,
        "summary": summary,
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
            parser.error("invalid endpoint construction stage configuration")
        run(args)
    else:
        report = json.loads(args.report.read_text())
        report["metadata"]["report_path"] = str(args.report.resolve())
        validate(report, ROOT if args.require_current_sources else None)
        print(f"{args.report}: endpoint construction resource stage verified")


if __name__ == "__main__":
    main()
