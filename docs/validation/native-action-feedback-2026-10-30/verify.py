#!/usr/bin/env python3
"""Verify native action-feedback sources, logs and protocol artifacts."""

import gzip
import hashlib
import json
from pathlib import Path
import subprocess
import sys


HERE = Path(__file__).resolve().parent
ROOT = HERE.parents[2]


def digest(path):
    return hashlib.sha256(path.read_bytes()).hexdigest()


def require_digest(relative, expected):
    path = HERE / relative
    if not path.is_file() or digest(path) != expected:
        raise RuntimeError(f"artifact digest mismatch: {relative}")
    return path


def main():
    results = json.loads((HERE / "results.json").read_text())
    for gate in results["native_gates"].values():
        compressed = require_digest(gate["artifact"], gate["artifact_sha256"])
        if hashlib.sha256(gzip.decompress(compressed.read_bytes())).hexdigest() != gate["uncompressed_sha256"]:
            raise RuntimeError(f"uncompressed log digest mismatch: {gate['artifact']}")
    for name in ["workspace_clippy", "no_ros_gate"]:
        gate = results[name]
        compressed = require_digest(gate["artifact"], gate["artifact_sha256"])
        if hashlib.sha256(gzip.decompress(compressed.read_bytes())).hexdigest() != gate["uncompressed_sha256"]:
            raise RuntimeError(f"uncompressed log digest mismatch: {gate['artifact']}")

    reports = []
    for name in ["cyclonedds", "fastdds", "owned_native_same_binary"]:
        evidence = results["benchmark_protocol"][name]
        reports.append(require_digest(evidence["artifact"], evidence["artifact_sha256"]))
        require_digest(evidence["samples_artifact"], evidence["samples_sha256"])
    stage = results["allocation_stage"]
    require_digest(stage["artifact"], stage["artifact_sha256"])
    require_digest(stage["raw_log_artifact"], stage["raw_log_sha256"])
    require_digest(stage["probe_report_artifact"], stage["probe_report_sha256"])
    failure = require_digest(
        results["formal_performance"]["artifact"],
        results["formal_performance"]["artifact_sha256"],
    )
    failed_report = json.loads(failure.read_text())
    if (failed_report["metadata"]["isolation_gate"].get("measurement_started") is not False
            or failed_report["records"] or failed_report["summary"]):
        raise RuntimeError("formal isolation failure contains measurements")

    for relative, expected in results["source_sha256"].items():
        path = ROOT / relative
        if not path.is_file() or digest(path) != expected:
            raise RuntimeError(f"source digest mismatch: {relative}")

    subprocess.run(
        [sys.executable, str(ROOT / "tools/bench_ros/verify_action_feedback.py"),
         "--require-current-sources", *map(str, reports)],
        cwd=ROOT,
        check=True,
    )
    subprocess.run(
        [sys.executable, str(ROOT / "tools/bench_ros/native_action_feedback_stage.py"),
         "verify", "--require-current-sources", str(HERE / stage["artifact"])],
        cwd=ROOT,
        check=True,
    )
    print("native action-feedback validation artifacts verified")


if __name__ == "__main__":
    main()
