#!/usr/bin/env python3
"""Verify archived pipelined/fanout pub-sub reports and raw samples offline."""
import argparse
import gzip
import hashlib
import json
import math
from pathlib import Path
import statistics

from build import pipeline_workspace_sources
from pubsub_pipeline import (
    ALL_LATENCY_FIELDS, ECHO_FIELDS, GATE_FIELDS, PING_FIELDS, matrix_pairs, parse_cases,
    rclcpp_failures, regression_failures, snapshot_is_idle, validate_endpoint,
)


def verify_isolation_gate(metadata):
    gate = metadata.get("isolation_gate")
    if gate is None:
        return False
    cpu_ping, cpu_echo = metadata.get("cpu_ping"), metadata.get("cpu_echo")
    orchestrator = metadata.get("orchestrator_cpu_affinity")
    if (type(cpu_ping) is not int or type(cpu_echo) is not int or cpu_ping == cpu_echo
            or not isinstance(orchestrator, list) or not orchestrator
            or any(type(cpu) is not int for cpu in orchestrator)
            or len(orchestrator) != len(set(orchestrator))
            or cpu_ping in orchestrator or cpu_echo in orchestrator):
        raise RuntimeError("invalid pipeline CPU isolation metadata")
    preflight, runtime = gate.get("preflight", {}), gate.get("runtime", {})
    required = preflight.get("required_consecutive_samples")
    maximum = preflight.get("maximum_load1")
    forbidden = preflight.get("forbidden_process_names")
    samples = preflight.get("samples")
    if (type(required) is not int or required < 3 or not isinstance(maximum, (int, float))
            or not math.isfinite(maximum) or maximum < 0
            or not isinstance(forbidden, list) or not forbidden
            or not isinstance(samples, list) or len(samples) != required
            or any(not isinstance(sample, dict)
                   or not isinstance(sample.get("observed_utc"), str)
                   or not isinstance(sample.get("load1"), (int, float))
                   or not math.isfinite(sample["load1"])
                   or not isinstance(sample.get("competing_processes"), dict)
                   or not snapshot_is_idle(sample, maximum) for sample in samples)):
        raise RuntimeError("invalid pipeline isolation preflight")
    violations = runtime.get("violations")
    if not isinstance(violations, list):
        raise RuntimeError("invalid pipeline runtime isolation violations")
    runtime_passed = not violations
    runtime_maximum = runtime.get("maximum_allowed_load1")
    if (not isinstance(runtime_maximum, (int, float)) or not math.isfinite(runtime_maximum)
            or runtime_maximum != maximum
            or type(runtime.get("observation_count")) is not int
            or runtime["observation_count"] < 2
            or not isinstance(runtime.get("maximum_load1"), (int, float))
            or not math.isfinite(runtime["maximum_load1"])
            or (runtime_passed and runtime["maximum_load1"] > runtime_maximum)
            or any(not isinstance(item, dict)
                   or (not item.get("competing_processes")
                       and item.get("load1", 0) <= runtime_maximum)
                   for item in violations)
            or runtime.get("passed") is not runtime_passed
            or preflight.get("passed") is not True
            or gate.get("passed") is not runtime_passed):
        raise RuntimeError("invalid pipeline runtime isolation gate")
    return runtime_passed


def current_source_mismatches(metadata, root):
    declared = {}
    manifests = [metadata.get("source_sha256", {})]
    build_manifest = metadata.get("build_manifest", {})
    manifests.extend([
        build_manifest.get("candidate", {}),
        build_manifest.get("cpp_source_sha256", {}),
    ])
    if not all(isinstance(manifest, dict) for manifest in manifests):
        raise RuntimeError("invalid pipeline source manifest")
    for manifest in manifests:
        for name, digest in manifest.items():
            if (not isinstance(name, str) or not isinstance(digest, str)
                    or len(digest) != 64):
                raise RuntimeError("invalid pipeline source identity")
            previous = declared.setdefault(name, digest)
            if previous != digest:
                raise RuntimeError(f"conflicting pipeline source identity: {name}")
    if not declared:
        raise RuntimeError("pipeline source manifest is required")
    mismatches = []
    for name, expected in sorted(declared.items()):
        source = root / name
        observed = hashlib.sha256(source.read_bytes()).hexdigest() if source.is_file() else None
        if observed != expected:
            mismatches.append({"file": name, "expected": expected, "observed": observed})
    return mismatches


def missing_pipeline_sources(metadata, root):
    required = {str(path.relative_to(root)) for path in pipeline_workspace_sources(root)}
    required.update(f"tools/bench_ros/{name}" for name in [
        "CMakeLists.txt", "pubsub_pipeline.cpp", "pubsub_pipeline.py", "build.py",
        "verify_pubsub_pipeline.py", "test_pubsub_pipeline.py",
    ])
    manifests = [metadata.get("source_sha256", {})]
    build_manifest = metadata.get("build_manifest", {})
    manifests.extend([build_manifest.get("candidate", {}),
                      build_manifest.get("cpp_source_sha256", {})])
    declared = {name for manifest in manifests for name in manifest}
    return sorted(required - declared)


def verify_compatibility_manifest(path, source_root=None):
    manifest = json.loads(path.read_text())
    root = (Path(__file__).resolve().parents[2] if source_root is None
            else Path(source_root).resolve())
    mismatches = current_source_mismatches(
        {"source_sha256": manifest.get("source_sha256", {})}, root)
    if mismatches:
        raise RuntimeError(f"compatibility source manifest mismatches current tree: {mismatches}")
    gates = manifest.get("gates")
    if not isinstance(gates, dict) or not gates:
        raise RuntimeError("compatibility gates are required")
    verified_logs = set()
    for gate, evidence in gates.items():
        if not isinstance(evidence, dict):
            raise RuntimeError(f"invalid compatibility gate: {gate}")
        relative, expected = evidence.get("log"), evidence.get("sha256")
        if not isinstance(relative, str) or not isinstance(expected, str) or len(expected) != 64:
            raise RuntimeError(f"invalid compatibility log identity: {gate}")
        log = path.parent / relative
        if not log.is_file() or hashlib.sha256(log.read_bytes()).hexdigest() != expected:
            raise RuntimeError(f"compatibility log digest mismatch: {gate}")
        verified_logs.add(relative)
    if len(verified_logs) != len(gates):
        raise RuntimeError("compatibility gates must use distinct logs")
    print(f"{path}: {len(manifest['source_sha256'])} source files and "
          f"{len(verified_logs)} compatibility logs verified")


def verify(path, require_performance_pass=False, require_isolation_pass=False,
           require_current_sources=False, source_root=None, require_rclcpp_pass=False):
    report = json.loads(path.read_text())
    metadata = report["metadata"]
    if metadata.get("workload") != "pubsub_pipeline":
        raise RuntimeError("unknown pipeline workload")
    cases = parse_cases(",".join(
        f"{case['elements']}:{case['window']}:{case['fanout']}"
        for case in metadata.get("cases", [])))
    iterations, warmup, repetitions = (
        metadata.get("iterations"), metadata.get("warmup"), metadata.get("repetitions"))
    if (type(iterations) is not int or iterations <= 0 or type(warmup) is not int or warmup < 0
            or type(repetitions) is not int or repetitions < 1
            or iterations + warmup > 2**32 - 1):
        raise RuntimeError("invalid pipeline run metadata")
    pairs = matrix_pairs(
        "baseline" in metadata.get("binary_sha256", {}), metadata.get("matrix_scope", "full"))
    receive_paths = metadata.get("receive_paths")
    if receive_paths is not None:
        expected_languages = {language for pair in pairs for language in pair}
        if (set(receive_paths) != set(metadata.get("binary_sha256", {}))
                or not expected_languages <= set(receive_paths)
                or receive_paths.get("cpp") != "callback"
                or any(path not in {"stream", "native_callback", "reusable_typed", "batch_typed", "callback"}
                       for path in receive_paths.values())):
            raise RuntimeError("invalid pipeline receive path metadata")
    artifact = metadata["samples_artifact"]
    compressed = (path.parent / artifact["file"]).read_bytes()
    if hashlib.sha256(compressed).hexdigest() != artifact["sha256"]:
        raise RuntimeError("pipeline sample artifact digest mismatch")
    traces = json.loads(gzip.decompress(compressed))
    records = report["records"]
    if len(traces) != len(records) or artifact.get("record_count") != len(records):
        raise RuntimeError("pipeline record/sample count mismatch")
    keys = set()
    for trace in traces:
        index = trace.get("record")
        if type(index) is not int or not 0 <= index < len(records):
            raise RuntimeError("invalid pipeline trace reference")
        record = records[index]
        key = (record["elements"], record["window"], record["fanout"],
               record["ping_language"], record["echo_language"], record["repetition"])
        if key in keys:
            raise RuntimeError("duplicate pipeline record")
        keys.add(key)
        ping = dict(record["ping"])
        echo = dict(record["echo"])
        ping["samples_ns"] = trace.get("samples_ns")
        ping["publish_samples_ns"] = trace.get("ping_publish_samples_ns")
        echo["collect_samples_ns"] = trace.get("echo_collect_samples_ns")
        echo["publish_samples_ns"] = trace.get("echo_publish_samples_ns")
        for endpoint, role in [(ping, "ping"), (echo, "echo")]:
            language = record[f"{role}_language"]
            if language not in {"rust", "cpp", "baseline"}:
                raise RuntimeError("unknown pipeline endpoint language")
            validate_endpoint(
                endpoint, role, metadata["rmw"], record["elements"], record["window"],
                record["fanout"], iterations, warmup,
                "rclcpp" if language == "cpp" else "r2r",
                None if receive_paths is None else receive_paths[language])
    expected = {(elements, window, fanout, ping, echo, repetition)
                for elements, window, fanout in cases for ping, echo in pairs
                for repetition in range(repetitions)}
    if keys != expected:
        raise RuntimeError("incomplete pipeline language/case matrix")
    summary_keys = set()
    for row in report["summary"]:
        identity = (row["elements"], row["window"], row["fanout"],
                    row["ping_language"], row["echo_language"])
        if identity in summary_keys:
            raise RuntimeError("duplicate pipeline summary row")
        summary_keys.add(identity)
        group = [record for record in records
                 if (record["elements"], record["window"], record["fanout"],
                     record["ping_language"], record["echo_language"]) == identity]
        if len(group) != repetitions:
            raise RuntimeError("missing pipeline repetition")
        for field in PING_FIELDS + ("messages_per_second",):
            if row.get(field) != statistics.median(record["ping"][field] for record in group):
                raise RuntimeError("pipeline summary median mismatch")
        for field in ECHO_FIELDS:
            if row.get(f"echo_{field}") != statistics.median(
                    record["echo"][field] for record in group):
                raise RuntimeError("pipeline echo summary median mismatch")
    expected_summary = {(elements, window, fanout, ping, echo)
                        for elements, window, fanout in cases for ping, echo in pairs}
    if summary_keys != expected_summary:
        raise RuntimeError("missing pipeline summary row")
    if "regression_gate" in metadata:
        gate = metadata["regression_gate"]
        if (not isinstance(gate.get("maximum_ratio"), (int, float))
                or not math.isfinite(gate["maximum_ratio"])
                or gate["maximum_ratio"] < 1):
            raise RuntimeError("invalid pipeline regression ratio")
        declared = metadata.get("regression_fields")
        if declared is None:
            fields = ALL_LATENCY_FIELDS
        else:
            if (not isinstance(declared, list) or declared != [*GATE_FIELDS, "inverse_throughput"]):
                raise RuntimeError("invalid pipeline regression fields")
            fields = GATE_FIELDS
        failures = regression_failures(report["summary"], cases, gate["maximum_ratio"], fields)
        if gate.get("passed") != (not failures) or gate.get("failures") != failures:
            raise RuntimeError("recorded pipeline gate does not match measurements")
    if "rclcpp_gate" in metadata:
        gate = metadata["rclcpp_gate"]
        if (gate.get("fields") != [*GATE_FIELDS, "inverse_throughput"]
                or not isinstance(gate.get("maximum_ratio"), (int, float))
                or not math.isfinite(gate["maximum_ratio"])
                or gate["maximum_ratio"] <= 0):
            raise RuntimeError("invalid pipeline rclcpp gate")
        failures = rclcpp_failures(
            report["summary"], cases, gate["maximum_ratio"], GATE_FIELDS)
        if gate.get("passed") != (not failures) or gate.get("failures") != failures:
            raise RuntimeError("recorded rclcpp gate does not match measurements")
    isolation_passed = verify_isolation_gate(metadata)
    if require_isolation_pass and not isolation_passed:
        raise RuntimeError("a recorded passing pipeline isolation gate is required")
    if require_performance_pass and (
            "regression_gate" not in metadata or not metadata["regression_gate"]["passed"]
            or metadata["regression_gate"]["maximum_ratio"] > 1.05):
        raise RuntimeError("a recorded passing pipeline regression gate is required")
    if require_performance_pass and not isolation_passed:
        raise RuntimeError("a passing performance report also requires isolation evidence")
    if require_rclcpp_pass and (
            "rclcpp_gate" not in metadata or not metadata["rclcpp_gate"]["passed"]
            or metadata["rclcpp_gate"]["maximum_ratio"] > 1.0):
        raise RuntimeError("a recorded passing rclcpp leadership gate is required")
    if require_rclcpp_pass and not isolation_passed:
        raise RuntimeError("a passing rclcpp report also requires isolation evidence")
    if require_current_sources:
        root = (Path(__file__).resolve().parents[2] if source_root is None
                else Path(source_root).resolve())
        mismatches = current_source_mismatches(metadata, root)
        if mismatches:
            raise RuntimeError(f"pipeline source manifest mismatches current tree: {mismatches}")
        missing = missing_pipeline_sources(metadata, root)
        if missing:
            raise RuntimeError(f"pipeline source manifest omits required inputs: {missing}")
    print(f"{path}: {len(records)} records and "
          f"{sum(len(trace['samples_ns']) for trace in traces)} message RTT samples verified")


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("report", type=Path, nargs="*")
    parser.add_argument("--require-performance-pass", action="store_true")
    parser.add_argument("--require-isolation-pass", action="store_true")
    parser.add_argument("--require-current-sources", action="store_true")
    parser.add_argument("--require-rclcpp-pass", action="store_true")
    parser.add_argument("--compatibility-manifest", type=Path)
    args = parser.parse_args()
    if not args.report and args.compatibility_manifest is None:
        parser.error("provide at least one report or a compatibility manifest")
    for path in args.report:
        verify(path, args.require_performance_pass, args.require_isolation_pass,
               args.require_current_sources, require_rclcpp_pass=args.require_rclcpp_pass)
    if args.compatibility_manifest is not None:
        verify_compatibility_manifest(args.compatibility_manifest)


if __name__ == "__main__":
    main()
