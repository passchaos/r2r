#!/usr/bin/env python3
"""Verify callback-group task sources and compressed gates."""

import gzip
import hashlib
import json
from pathlib import Path


HERE = Path(__file__).resolve().parent
ROOT = HERE.parents[2]


def digest(data):
    return hashlib.sha256(data).hexdigest()


def main():
    results = json.loads((HERE / "results.json").read_text())
    gates = [*results["native_gates"].values(), results["workspace_clippy"], results["no_ros_gate"]]
    for gate in gates:
        artifact = HERE / gate["artifact"]
        compressed = artifact.read_bytes()
        if digest(compressed) != gate["artifact_sha256"]:
            raise RuntimeError(f"compressed log digest mismatch: {artifact}")
        if digest(gzip.decompress(compressed)) != gate["uncompressed_sha256"]:
            raise RuntimeError(f"uncompressed log digest mismatch: {artifact}")
    for relative, expected in results["source_sha256"].items():
        source = ROOT / relative
        if not source.is_file() or digest(source.read_bytes()) != expected:
            raise RuntimeError(f"source digest mismatch: {relative}")
    jazzy = results["native_gates"]["jazzy"]
    if (jazzy["task_cases_passed_per_rmw_release"] != 8
            or jazzy["reentrant_scheduler_cases_passed_per_rmw_release"] != 2
            or jazzy["action_cases_passed_per_rmw_release"] != 4
            or jazzy["examples_passed_per_rmw"] != 2
            or results["no_ros_gate"]["public_documentation_artifacts"] != 84
            or results["no_ros_gate"]["required_documented_methods"] != 341
            or not results["feature"]["reentrant_concurrency_bounded"]):
        raise RuntimeError("callback-group task gate counts changed")
    print("callback-group task validation artifacts verified")


if __name__ == "__main__":
    main()
