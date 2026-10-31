#!/usr/bin/env python3
"""Verify typed batch pipeline protocol, provenance and isolation failure."""

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
        manifest = data["metadata"].get("build_manifest", {})
        if (len(data["records"]) != 12
                or data["metadata"].get("receive_paths")
                != {"rust": "batch_typed", "cpp": "callback"}
                or manifest.get("requested_receive_path") != "batch_typed"
                or "subscribe_batches" not in manifest.get("batch_typed_api_detection", "")):
            raise RuntimeError("typed batch pipeline record/path provenance mismatch")
        reports.append(report)
    for evidence in results["batch_reusable_protocol"].values():
        report = require(evidence["artifact"], evidence["artifact_sha256"])
        require(evidence["samples_artifact"], evidence["samples_sha256"])
        data = json.loads(report.read_text())
        if (len(data["records"]) != 6
                or data["metadata"].get("matrix_scope") != "regression"
                or data["metadata"].get("receive_paths")
                != {"rust": "batch_typed", "cpp": "callback", "baseline": "reusable_typed"}):
            raise RuntimeError("batch/reusable protocol path mismatch")
        reports.append(report)
    require("build-manifest.json", results["build_manifest_sha256"])
    require(
        "reusable-typed-build-manifest.json",
        results["reusable_typed_build_manifest_sha256"],
    )
    failure = json.loads(require(
        results["formal_performance"]["artifact"],
        results["formal_performance"]["artifact_sha256"],
    ).read_text())
    if (failure["metadata"]["isolation_gate"].get("measurement_started") is not False
            or failure["metadata"].get("receive_paths")
            != {"rust": "batch_typed", "cpp": "callback", "baseline": "reusable_typed"}
            or failure["metadata"].get("binary_sha256", {}).get("baseline")
            != results["reusable_typed_binary_sha256"]
            or failure["records"] or failure["summary"]):
        raise RuntimeError("typed batch isolation failure contains measurements")
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
    print("typed batch pipeline validation artifacts verified")


if __name__ == "__main__":
    main()
