#!/usr/bin/env python3
"""Verify reusable typed pipeline protocol, sources and isolation failure."""

import hashlib
import json
from pathlib import Path
import subprocess
import sys


HERE = Path(__file__).resolve().parent
ROOT = HERE.parents[2]


def digest(path):
    return hashlib.sha256(path.read_bytes()).hexdigest()


def require(relative, expected):
    path = HERE / relative
    if not path.is_file() or digest(path) != expected:
        raise RuntimeError(f"artifact digest mismatch: {relative}")
    return path


def main():
    results = json.loads((HERE / "results.json").read_text())
    reports = []
    for evidence in results["protocol"].values():
        report = require(evidence["artifact"], evidence["artifact_sha256"])
        require(evidence["samples_artifact"], evidence["samples_sha256"])
        data = json.loads(report.read_text())
        if (len(data["records"]) != 12 or data["metadata"]["receive_paths"]
                != {"rust": "reusable_typed", "cpp": "callback"}):
            raise RuntimeError("reusable typed pipeline record/path mismatch")
        reports.append(report)
    require("build-manifest.json", results["build_manifest_sha256"])
    failure = json.loads(require(
        results["formal_performance"]["artifact"],
        results["formal_performance"]["artifact_sha256"],
    ).read_text())
    if (failure["metadata"]["isolation_gate"].get("measurement_started") is not False
            or failure["records"] or failure["summary"]):
        raise RuntimeError("pipeline isolation failure contains measurements")
    for relative, expected in results["source_sha256"].items():
        source = ROOT / relative
        if not source.is_file() or digest(source) != expected:
            raise RuntimeError(f"source digest mismatch: {relative}")
    subprocess.run(
        [sys.executable, str(ROOT / "tools/bench_ros/verify_pubsub_pipeline.py"),
         "--require-current-sources", *map(str, reports)],
        cwd=ROOT,
        check=True,
    )
    print("reusable typed pipeline validation artifacts verified")


if __name__ == "__main__":
    main()
