#!/usr/bin/env python3
"""Verify archived idle-spin resource scaling evidence."""

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


def main():
    results = json.loads((HERE / "results.json").read_text())
    reports = []
    for evidence in results["reports"].values():
        artifact = HERE / evidence["artifact"]
        if not artifact.is_file() or digest(artifact) != evidence["artifact_sha256"]:
            raise RuntimeError(f"artifact digest mismatch: {evidence['artifact']}")
        reports.append(artifact)
    stage = results["allocation_stage"]
    require = HERE / stage["probe_report_artifact"]
    if not require.is_file() or digest(require) != stage["probe_report_sha256"]:
        raise RuntimeError("probe report digest mismatch")
    for relative, expected in results["source_sha256"].items():
        source = ROOT / relative
        if not source.is_file() or digest(source) != expected:
            raise RuntimeError(f"source digest mismatch: {relative}")
    for report in reports:
        subprocess.run(
            [sys.executable, str(ROOT / "tools/bench_ros/idle_spin_resource_stage.py"),
             "verify", "--require-current-sources", str(report)],
            cwd=ROOT,
            check=True,
        )
    print("idle-spin resource scaling evidence verified")


if __name__ == "__main__":
    main()
