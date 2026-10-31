#!/usr/bin/env python3
"""Verify batched-subscription sources and compressed validation logs."""

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


def verified_log(gate):
    path = HERE / gate["artifact"]
    if not path.is_file() or digest(path) != gate["artifact_sha256"]:
        raise RuntimeError(f"artifact digest mismatch: {gate['artifact']}")
    raw = gzip.decompress(path.read_bytes())
    if hashlib.sha256(raw).hexdigest() != gate["uncompressed_sha256"]:
        raise RuntimeError(f"uncompressed log digest mismatch: {gate['artifact']}")
    return raw.decode(errors="replace")


def require_count(text, fragment, expected, gate):
    actual = text.count(fragment)
    if actual != expected:
        raise RuntimeError(f"{gate}: expected {expected} occurrences of {fragment!r}, got {actual}")


def main():
    results = json.loads((HERE / "results.json").read_text())
    logs = {
        name: verified_log(gate)
        for name, gate in {
            **results["native_gates"],
            "workspace_clippy": results["workspace_clippy"],
            "no_ros_gate": results["no_ros_gate"],
        }.items()
    }
    require_count(
        logs["jazzy"], "6 passed; 0 failed; 2 ignored", 4, "jazzy batch debug/release"
    )
    require_count(logs["jazzy"], "2 passed; 0 failed", 2, "jazzy native reset")
    require_count(logs["jazzy"], "1 passed; 0 failed", 3, "jazzy filter/generator")
    require_count(logs["jazzy"], "received four ordered messages", 2, "jazzy example")
    require_count(logs["humble"], "6 passed; 0 failed; 2 ignored", 1, "humble batch")
    require_count(logs["humble"], "2 passed; 0 failed", 1, "humble native reset")
    require_count(logs["humble"], "1 passed; 0 failed", 2, "humble filter/generator")
    require_count(logs["humble"], "received four ordered messages", 1, "humble example")
    require_count(logs["kilted"], "6 passed; 0 failed; 2 ignored", 2, "kilted batch")
    require_count(logs["kilted"], "2 passed; 0 failed", 2, "kilted native reset")
    require_count(logs["kilted"], "1 passed; 0 failed", 3, "kilted filter/generator")
    require_count(logs["kilted"], "received four ordered messages", 2, "kilted example")
    require_count(logs["no_ros_gate"], "PASS public documentation:", 84, "no-ROS pages")
    require_count(logs["no_ros_gate"], "PASS documented method:", 341, "no-ROS methods")
    if "84 public documentation artifacts verified without ROS" not in logs["no_ros_gate"]:
        raise RuntimeError("no-ROS documentation inventory marker missing")
    if "1 passed; 0 failed" not in logs["no_ros_gate"]:
        raise RuntimeError("batch Rustdoc result missing")
    if results["no_ros_gate"]["public_documentation_artifacts"] != 84:
        raise RuntimeError("public documentation artifact count changed")
    if results["no_ros_gate"]["required_documented_methods"] != 341:
        raise RuntimeError("documented method count changed")
    if not results["workspace_clippy"]["warnings_denied"]:
        raise RuntimeError("workspace Clippy did not deny warnings")
    stage = results["reset_allocation_stage"]
    report = HERE / stage["artifact"]
    raw = HERE / stage["raw_log_artifact"]
    if (not report.is_file() or digest(report) != stage["artifact_sha256"]
            or not raw.is_file() or digest(raw) != stage["raw_log_sha256"]):
        raise RuntimeError("native batch reset stage artifact mismatch")
    for relative, expected in results["source_sha256"].items():
        source = ROOT / relative
        if not source.is_file() or digest(source) != expected:
            raise RuntimeError(f"source digest mismatch: {relative}")
    subprocess.run(
        [sys.executable, str(ROOT / "tools/bench_ros/native_batch_reset_stage.py"),
         "verify", "--require-current-sources", str(report)],
        cwd=ROOT,
        check=True,
    )
    sparse = results["sparse_reset_stage"]
    sparse_report = HERE / sparse["artifact"]
    sparse_raw = HERE / sparse["raw_log_artifact"]
    if (
        not sparse_report.is_file()
        or digest(sparse_report) != sparse["artifact_sha256"]
        or not sparse_raw.is_file()
        or digest(sparse_raw) != sparse["raw_log_sha256"]
    ):
        raise RuntimeError("sparse native batch reset stage artifact mismatch")
    subprocess.run(
        [
            sys.executable,
            str(ROOT / "tools/bench_ros/native_batch_sparse_reset_stage.py"),
            "verify",
            "--require-current-sources",
            str(sparse_report),
        ],
        cwd=ROOT,
        check=True,
    )
    print("batched subscription validation artifacts verified")


if __name__ == "__main__":
    main()
