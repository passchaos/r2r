#!/usr/bin/env python3
"""Verify reusable typed-subscription sources, logs and allocation evidence."""

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
    for gate in [
        *results["native_gates"].values(),
        *results["full_release"]["rmws"].values(),
        results["workspace_clippy"],
        results["no_ros_gate"],
    ]:
        artifact = require_digest(gate["artifact"], gate["artifact_sha256"])
        if hashlib.sha256(gzip.decompress(artifact.read_bytes())).hexdigest() != gate["uncompressed_sha256"]:
            raise RuntimeError(f"uncompressed log digest mismatch: {gate['artifact']}")
    stage = results["allocation_stage"]
    report = require_digest(stage["artifact"], stage["artifact_sha256"])
    require_digest(stage["raw_log_artifact"], stage["raw_log_sha256"])
    require_digest(stage["probe_report_artifact"], stage["probe_report_sha256"])
    default_stage = results["default_typed_native_reuse_stage"]
    default_report = require_digest(default_stage["artifact"], default_stage["artifact_sha256"])
    require_digest(default_stage["raw_log_artifact"], default_stage["raw_log_sha256"])
    for relative, expected in results["source_sha256"].items():
        source = ROOT / relative
        if not source.is_file() or digest(source) != expected:
            raise RuntimeError(f"source digest mismatch: {relative}")
    if (results["native_gates"]["jazzy"]["generator_cases_passed_per_rmw_release"] != 2
            or results["full_release"]["passed_per_rmw"] != 822
            or results["full_release"]["ignored_per_rmw"] != 20
            or results["native_gates"]["humble"]["generator_cases_passed_per_rmw"] != 4
            or results["no_ros_gate"]["public_documentation_artifacts"] != 84
            or results["no_ros_gate"]["required_documented_methods"] != 341
            or stage["fresh_allocations"] != 30_000
            or stage["reusable_allocations"] != 20_000
            or stage["requested_byte_reduction"] != 327_600_000
            or default_stage["fresh_native_allocations"] != 50_000
            or default_stage["reused_native_allocations"] != 30_000
            or default_stage["requested_byte_reduction"] != 328_240_000):
        raise RuntimeError("reusable typed subscription gate counts changed")
    subprocess.run(
        [sys.executable, str(ROOT / "tools/bench_ros/typed_subscription_reuse_stage.py"),
         "verify", "--require-current-sources", str(report)],
        cwd=ROOT,
        check=True,
    )
    subprocess.run(
        [sys.executable, str(ROOT / "tools/bench_ros/typed_subscription_native_reuse_stage.py"),
         "verify", "--require-current-sources", str(default_report)],
        cwd=ROOT,
        check=True,
    )
    print("reusable typed subscription validation artifacts verified")


if __name__ == "__main__":
    main()
