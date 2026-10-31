#!/usr/bin/env python3
"""Verify reusable native-subscription sources, logs and allocation evidence."""

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
    for gate in [*results["native_gates"].values(), results["workspace_clippy"], results["no_ros_gate"]]:
        artifact = require_digest(gate["artifact"], gate["artifact_sha256"])
        if hashlib.sha256(gzip.decompress(artifact.read_bytes())).hexdigest() != gate["uncompressed_sha256"]:
            raise RuntimeError(f"uncompressed log digest mismatch: {gate['artifact']}")
    stage = results["allocation_stage"]
    report = require_digest(stage["artifact"], stage["artifact_sha256"])
    require_digest(stage["raw_log_artifact"], stage["raw_log_sha256"])
    require_digest(stage["probe_report_artifact"], stage["probe_report_sha256"])
    for relative, expected in results["source_sha256"].items():
        source = ROOT / relative
        if not source.is_file() or digest(source) != expected:
            raise RuntimeError(f"source digest mismatch: {relative}")
    if (results["native_gates"]["jazzy"]["reusable_cases_passed_per_rmw_release"] != 1
            or results["no_ros_gate"]["public_documentation_artifacts"] != 84
            or results["no_ros_gate"]["required_documented_methods"] != 341
            or stage["fresh_allocations"] != 40_000
            or stage["reusable_allocations"] != 20_000
            or stage["requested_byte_reduction"] != 328_080_000):
        raise RuntimeError("reusable native subscription gate counts changed")
    subprocess.run(
        [sys.executable, str(ROOT / "tools/bench_ros/native_subscription_reuse_stage.py"),
         "verify", "--require-current-sources", str(report)],
        cwd=ROOT,
        check=True,
    )
    print("reusable native subscription validation artifacts verified")


if __name__ == "__main__":
    main()
