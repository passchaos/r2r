#!/usr/bin/env python3
"""Run independent Rust/rclcpp binaries in alternating order on one CPU."""
import argparse
import datetime
import hashlib
import json
import math
import os
from pathlib import Path
import platform
import statistics
import subprocess
import tempfile
import xml.etree.ElementTree as ET


def run(command, env):
    return subprocess.run(command, env=env, check=True, text=True, capture_output=True, timeout=120).stdout.strip()


def validate_result(result, rmw, mode, size, iterations, wire_bytes, wire_hash):
    if (result["rmw"], result["mode"], result["elements"], result["iterations"]) != (rmw, mode, size, iterations):
        raise RuntimeError("benchmark identity mismatch")
    if (result["wire_bytes"], result["wire_hash"]) != (wire_bytes, wire_hash):
        raise RuntimeError("benchmark wire fixture mismatch")
    metrics = [result[key] for key in ["ns_per_op", "batch_ns_per_op_p50", "batch_ns_per_op_p95", "batch_ns_per_op_p99"]]
    if result["elapsed_ns"] <= 0 or any(not math.isfinite(value) or value <= 0 for value in metrics):
        raise RuntimeError("invalid benchmark timing")
    if iterations <= 0 or not math.isclose(result["ns_per_op"], result["elapsed_ns"] / iterations, rel_tol=1e-5):
        raise RuntimeError("benchmark operation-time calculation mismatch")


def regression_failures(summary, maximum):
    return [row for row in summary if row["median_ns_per_op"]["candidate"] / row["median_ns_per_op"]["baseline"] > maximum]


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--rust", action="append", required=True, metavar="LABEL=EXECUTABLE")
    parser.add_argument("--cpp", type=Path, required=True)
    parser.add_argument("--cpu", type=int, required=True)
    parser.add_argument("--rmw", default="rmw_cyclonedds_cpp")
    parser.add_argument("--sizes", default="0,32,4096")
    parser.add_argument("--modes", default="copy_reuse,serialize_owned,deserialize_owned")
    parser.add_argument("--iterations", type=int, default=100_000)
    parser.add_argument("--repetitions", type=int, default=5)
    parser.add_argument("--minimum-run-ms", type=int, default=200)
    parser.add_argument("--output", type=Path, required=True)
    parser.add_argument("--build-manifest", type=Path)
    parser.add_argument("--max-regression", type=float, help="Maximum candidate/baseline median ratio; requires those labels")
    args = parser.parse_args()
    if args.cpu not in os.sched_getaffinity(0):
        parser.error("CPU is outside the allowed affinity mask")
    if args.iterations <= 0 or args.repetitions < 3:
        parser.error("iterations must be positive and repetitions must be at least three")
    if args.minimum_run_ms < 0:
        parser.error("minimum run duration cannot be negative")
    sizes = [int(size) for size in args.sizes.split(",")]
    modes = args.modes.split(",")
    if not modes or any(mode not in {"copy_reuse", "serialize_owned", "serialize_reuse", "serialize_owned_reuse", "deserialize_owned"} for mode in modes) or len(set(modes)) != len(modes):
        parser.error("invalid or duplicate benchmark modes")
    if any(size < 0 or size > 2**32 - 1 for size in sizes):
        parser.error("invalid message sizes")
    binaries = {}
    for value in args.rust:
        label, executable = value.split("=", 1)
        if label in binaries or label == "rclcpp":
            parser.error("binary labels must be unique and cannot use rclcpp")
        binaries[label] = Path(executable).resolve()
    binaries["rclcpp"] = args.cpp.resolve()
    if args.max_regression is not None and (not math.isfinite(args.max_regression) or args.max_regression < 1 or not {"baseline", "candidate"}.issubset(binaries)):
        parser.error("a regression gate requires baseline/candidate labels and a ratio at least one")
    env = os.environ.copy()
    env["RMW_IMPLEMENTATION"] = args.rmw
    # This comparison uses the middleware codec and does not create a DDS domain.
    env.pop("CYCLONEDDS_URI", None)
    metadata = {
        "started_utc": datetime.datetime.now(datetime.timezone.utc).isoformat(),
        "cpu": args.cpu, "rmw": args.rmw, "ros_distro": env.get("ROS_DISTRO"),
        "platform": platform.platform(),
        "cpu_model": next((line.split(":", 1)[1].strip() for line in Path("/proc/cpuinfo").read_text().splitlines() if line.startswith("model name")), "unknown"),
        "rustc": run(["rustc", "--version"], env),
        "cxx": run(["c++", "--version"], env).splitlines()[0],
        "git_head": run(["git", "rev-parse", "HEAD"], env),
        "working_tree_dirty": bool(run(["git", "status", "--porcelain"], env)),
        "binary_sha256": {label: hashlib.sha256(path.read_bytes()).hexdigest() for label, path in binaries.items()},
        "source_sha256": {str(path): hashlib.sha256(path.read_bytes()).hexdigest() for path in [Path("r2r/benches/ros_comparison.rs"), Path("tools/bench_ros/comparison.cpp")]},
        "build_settings": {"rust": "cargo --release (opt-level 3)", "cxx": "-O3 -DNDEBUG C++17", "target_cpu": "compiler default; no target-native override"},
        "rustflags": env.get("RUSTFLAGS", ""),
        "cargo_release_opt_level": env.get("CARGO_PROFILE_RELEASE_OPT_LEVEL", "3"),
        "semantics": "same Int32MultiArray fixture; full payload and wire-byte oracle; copy into reused destination; serialize_owned returns freshly owned bytes; serialize_reuse consumes a borrowed result when MessageSerializer exists (historical owned-byte fallback otherwise); serialize_owned_reuse copies that result to fresh owned bytes; deserialization returns freshly owned message",
        "latency_samples": "batch elapsed time divided by batch operation count; not individual-message tail latency",
        "minimum_iterations": args.iterations,
        "minimum_run_ms": args.minimum_run_ms,
        "modes": modes,
        "payload_sizes": sizes,
        "repetitions": args.repetitions,
    }
    prefix = Path("/opt/ros") / env.get("ROS_DISTRO", "unknown") / "share"
    if args.build_manifest:
        metadata["build_manifest"] = json.loads(args.build_manifest.read_text())
    metadata["ros_packages"] = {name: ET.parse(prefix / name / "package.xml").findtext("version") for name in ["rclcpp", "rcl", "rmw", "std_msgs", args.rmw]}
    records = []
    with tempfile.TemporaryDirectory(prefix="r2r-ros-comparison-") as directory:
        for size in sizes:
            expected = None
            wire_hash = None
            for label, binary in binaries.items():
                fixture = Path(directory) / f"{label}-{size}.cdr"
                result = json.loads(run(["taskset", "-c", str(args.cpu), str(binary), "copy_reuse", str(size), "1", str(fixture)], env))
                wire = fixture.read_bytes()
                if expected is not None and wire != expected:
                    raise RuntimeError(f"wire oracle mismatch: {label}, size {size}")
                if result["rmw"] != args.rmw:
                    raise RuntimeError(f"RMW mismatch: {label}, {result['rmw']}")
                if wire_hash is not None and result["wire_hash"] != wire_hash:
                    raise RuntimeError(f"wire checksum mismatch: {label}, size {size}")
                expected = wire
                wire_hash = result["wire_hash"]
            for mode in modes:
                calibrations = [json.loads(run(["taskset", "-c", str(args.cpu), str(binary), mode, str(size), "10000"], env))["ns_per_op"] for binary in binaries.values()]
                iterations = max(args.iterations, math.ceil(args.minimum_run_ms * 1_000_000 / min(calibrations)))
                expected_checksum = None
                for repetition in range(args.repetitions):
                    order = list(binaries) if repetition % 2 == 0 else list(reversed(binaries))
                    for label in order:
                        result = json.loads(run(["taskset", "-c", str(args.cpu), str(binaries[label]), mode, str(size), str(iterations)], env))
                        validate_result(result, args.rmw, mode, size, iterations, len(expected), wire_hash)
                        if expected_checksum is not None and result["checksum"] != expected_checksum:
                            raise RuntimeError(f"result oracle mismatch: {label}, {mode}, {size}")
                        expected_checksum = result["checksum"]
                        result.update(label=label, repetition=repetition)
                        records.append(result)
                    print(f"{mode} elements={size} repetition={repetition + 1}/{args.repetitions}", flush=True)
    summary = []
    for size in sizes:
        for mode in modes:
            medians = {label: statistics.median(record["ns_per_op"] for record in records if record["label"] == label and record["elements"] == size and record["mode"] == mode) for label in binaries}
            summary.append({"elements": size, "mode": mode, "median_ns_per_op": medians,
                            "relative_to_rclcpp": {label: median / medians["rclcpp"] for label, median in medians.items()}})
    args.output.parent.mkdir(parents=True, exist_ok=True)
    regressions = regression_failures(summary, args.max_regression) if args.max_regression is not None else []
    if args.max_regression is not None:
        metadata["regression_gate"] = {"maximum_ratio": args.max_regression, "passed": not regressions, "failures": regressions}
    args.output.write_text(json.dumps({"metadata": metadata, "records": records, "summary": summary}, indent=2) + "\n")
    print(json.dumps(summary, indent=2))
    if args.max_regression is not None:
        if regressions:
            raise RuntimeError(f"performance regression gate failed: {regressions}")


if __name__ == "__main__":
    main()
