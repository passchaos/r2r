#!/usr/bin/env python3
"""Verify the archived pipeline peak-RSS comparison against its protocol reports."""

import hashlib
import json
from pathlib import Path
import subprocess
import sys
import tempfile


HERE = Path(__file__).resolve().parent
ROOT = HERE.parents[2]


def digest(path):
    return hashlib.sha256(path.read_bytes()).hexdigest()


def main():
    results = json.loads((HERE / "results.json").read_text())
    for relative, expected in results["protocol_sha256"].items():
        source = ROOT / relative
        if not source.is_file() or digest(source) != expected:
            raise RuntimeError(f"protocol digest mismatch: {relative}")
    comparison = HERE / "comparison.json"
    if digest(comparison) != results["comparison_sha256"]:
        raise RuntimeError("comparison digest mismatch")
    with tempfile.TemporaryDirectory() as directory:
        regenerated = Path(directory) / "comparison.json"
        subprocess.run(
            [sys.executable, str(ROOT / "tools/bench_ros/pipeline_resource_comparison.py"),
             "--cyclonedds", str(ROOT / "docs/validation/reusable-typed-pipeline-2026-10-31/protocol/cyclonedds.json"),
             "--fastdds", str(ROOT / "docs/validation/reusable-typed-pipeline-2026-10-31/protocol/fastdds.json"),
             "--output", str(regenerated)],
            cwd=ROOT,
            check=True,
            stdout=subprocess.DEVNULL,
        )
        if digest(regenerated) != results["comparison_sha256"]:
            raise RuntimeError("comparison does not reproduce from archived protocols")
    print("pipeline peak-RSS comparison verified")


if __name__ == "__main__":
    main()
