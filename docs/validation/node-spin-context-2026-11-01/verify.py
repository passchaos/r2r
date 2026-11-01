#!/usr/bin/env python3
"""Verify archived Node blocking-spin and shared-context evidence."""

import gzip
import hashlib
import json
from pathlib import Path


HERE = Path(__file__).resolve().parent


def digest(path):
    return hashlib.sha256(path.read_bytes()).hexdigest()


def main():
    results = json.loads((HERE / "results.json").read_text())
    for name, gate in results["gates"].items():
        artifact = HERE / gate["artifact"]
        if not artifact.is_file() or digest(artifact) != gate["artifact_sha256"]:
            raise RuntimeError(f"artifact digest mismatch: {gate['artifact']}")
        if hashlib.sha256(gzip.decompress(artifact.read_bytes())).hexdigest() != gate["uncompressed_sha256"]:
            raise RuntimeError(f"uncompressed log digest mismatch: {gate['artifact']}")
    if results["full_release"]["passed_per_rmw"] != 838:
        raise RuntimeError("unexpected full-release pass count")
    if results["full_release"]["ignored_per_rmw"] != 25:
        raise RuntimeError("unexpected full-release ignored count")
    if results["no_ros_gate"]["public_documentation_artifacts"] != 84:
        raise RuntimeError("unexpected documentation artifact count")
    if results["no_ros_gate"]["required_documented_methods"] != 341:
        raise RuntimeError("unexpected documented method count")
    print("Node blocking-spin and shared-context evidence verified")


if __name__ == "__main__":
    main()
