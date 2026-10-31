#!/usr/bin/env python3
"""Verify archived concurrent action-feedback matrices and raw samples offline."""
import argparse
import gzip
import hashlib
import json
from pathlib import Path
import statistics

from action_feedback import (
    SERVER_STAGES,
    matrix_pairs,
    rclcpp_failures,
    regression_failures,
    validate_endpoint,
)
from callback_group_support.validation import qualified_isolation
from pubsub_pipeline import DEFAULT_FORBIDDEN_PROCESS_NAMES


def current_source_mismatches(metadata, root):
    declared = {}
    build_manifest = metadata.get("build_manifest", {})
    manifests = [metadata.get("source_sha256", {}),
                 build_manifest.get("candidate", {}),
                 build_manifest.get("cpp_source_sha256", {})]
    if not all(isinstance(manifest, dict) for manifest in manifests):
        raise RuntimeError("invalid action feedback source manifest")
    for manifest in manifests:
        for name, digest in manifest.items():
            if not isinstance(name, str) or not isinstance(digest, str) or len(digest) != 64:
                raise RuntimeError("invalid action feedback source identity")
            previous = declared.setdefault(name, digest)
            if previous != digest:
                raise RuntimeError(f"conflicting action feedback source identity: {name}")
    required = {
        "r2r/benches/action_feedback_roundtrip.rs",
        "tools/bench_ros/action_feedback.cpp",
        "tools/bench_ros/action_feedback.py",
        "tools/bench_ros/verify_action_feedback.py",
        "tools/bench_ros/pubsub_pipeline.py",
        "tools/bench_ros/callback_group_support/validation.py",
    }
    if not required <= set(declared):
        raise RuntimeError(f"action feedback source manifest omits: {sorted(required-set(declared))}")
    mismatches = []
    for name, expected in sorted(declared.items()):
        source = root / name
        observed = hashlib.sha256(source.read_bytes()).hexdigest() if source.is_file() else None
        if observed != expected:
            mismatches.append({"file": name, "expected": expected, "observed": observed})
    return mismatches


def verify(path, require_performance_pass=False, require_current_sources=False):
    report = json.loads(path.read_text())
    metadata = report["metadata"]
    if metadata.get("workload") != "action_feedback":
        raise RuntimeError("unknown action feedback workload")
    sizes, windows = metadata.get("payload_sizes"), metadata.get("goal_windows")
    feedback_per_goal = metadata.get("feedback_per_goal")
    if (not isinstance(sizes, list) or not sizes or len(set(sizes)) != len(sizes)
            or any(type(value) is not int or not 0 <= value <= 1_000_000 for value in sizes)
            or not isinstance(windows, list) or not windows or len(set(windows)) != len(windows)
            or any(type(value) is not int or not 1 <= value <= 64 for value in windows)
            or type(feedback_per_goal) is not int or not 1 <= feedback_per_goal <= 256
            or max(windows) * feedback_per_goal > 4096):
        raise RuntimeError("invalid action feedback metadata dimensions")
    instrumentation = metadata.get("instrumentation")
    if (not isinstance(instrumentation, dict)
            or type(instrumentation.get("enabled")) is not bool
            or instrumentation.get("latency_evidence") is not (not instrumentation["enabled"])):
        raise RuntimeError("invalid action feedback instrumentation metadata")
    server_stages = "server_stage_labels" in metadata
    if server_stages and metadata["server_stage_labels"] != list(SERVER_STAGES):
        raise RuntimeError("invalid action feedback server stage metadata")
    status_qos = metadata.get("status_qos")
    if status_qos is not None and status_qos not in {
        "default", "reliable_volatile", "best_effort_volatile"
    }:
        raise RuntimeError("invalid action feedback status QoS metadata")
    result_timeout_ms = metadata.get("result_timeout_ms")
    if result_timeout_ms is not None and (
        type(result_timeout_ms) is not int or not 1 <= result_timeout_ms <= 600_000
    ):
        raise RuntimeError("invalid action feedback result timeout metadata")
    if status_qos is not None and result_timeout_ms is not None:
        expected_qos = (
            "goal/result/cancel/feedback reliable volatile "
            "KeepLast(max(16, goal_window*feedback_per_goal)); "
            f"status {status_qos}; result retention {result_timeout_ms} ms"
        )
        if metadata.get("qos") != expected_qos:
            raise RuntimeError("action feedback QoS description mismatch")
    status_batch_size = metadata.get("status_batch_size")
    if status_batch_size is not None and (
        type(status_batch_size) is not int
        or not 1 <= status_batch_size <= min(windows)
    ):
        raise RuntimeError("invalid action feedback status batch metadata")
    pairs = matrix_pairs(
        "baseline" in metadata.get("binary_sha256", {}), metadata.get("matrix_scope", "full")
    )
    status_paths = metadata.get("status_paths")
    if status_paths is not None:
        expected_languages = {language for pair in pairs for language in pair}
        binary_languages = set(metadata.get("binary_sha256", {}))
        if (set(status_paths) != binary_languages
                or not expected_languages <= binary_languages
                or any(path not in {"per_goal", "batched"} for path in status_paths.values())
                or status_paths.get("cpp", "per_goal") != "per_goal"):
            raise RuntimeError("invalid action feedback status path metadata")
    status_sync = metadata.get("status_sync")
    if status_sync is not None and status_sync not in {"required", "none"}:
        raise RuntimeError("invalid action feedback status sync metadata")
    feedback_delivery_paths = metadata.get("feedback_delivery_paths")
    if feedback_delivery_paths is not None:
        expected_languages = {language for pair in pairs for language in pair}
        binary_languages = set(metadata.get("binary_sha256", {}))
        if (set(feedback_delivery_paths) != binary_languages
                or not expected_languages <= binary_languages
                or feedback_delivery_paths.get("cpp") != "rclcpp_callback"
                or any(path not in {"owned", "native", "rclcpp_callback"}
                       for path in feedback_delivery_paths.values())
                or any(path == "rclcpp_callback" and language != "cpp"
                       for language, path in feedback_delivery_paths.items())):
            raise RuntimeError("invalid action feedback delivery path metadata")
    artifact = metadata["samples_artifact"]
    compressed = (path.parent / artifact["file"]).read_bytes()
    if hashlib.sha256(compressed).hexdigest() != artifact["sha256"]:
        raise RuntimeError("action feedback sample artifact digest mismatch")
    traces = json.loads(gzip.decompress(compressed))
    records = report["records"]
    if len(traces) != len(records) or artifact.get("record_count") != len(records):
        raise RuntimeError("action feedback record/sample count mismatch")
    keys = set()
    for trace in traces:
        index = trace.get("record")
        if type(index) is not int or not 0 <= index < len(records):
            raise RuntimeError("invalid action feedback trace reference")
        record = records[index]
        key = (record["elements"], record["goal_window"], record["ping_language"],
               record["echo_language"], record["repetition"])
        if key in keys:
            raise RuntimeError("duplicate action feedback record")
        keys.add(key)
        for role in ["ping", "echo"]:
            endpoint = dict(record[role])
            if role == "ping":
                endpoint["samples_ns"] = trace["samples_ns"]
                endpoint["feedback_samples_ns"] = trace["feedback_samples_ns"]
            elif server_stages:
                endpoint["server_stage_samples_ns"] = trace.get("server_stage_samples_ns")
            language = record[f"{role}_language"]
            if language not in {"rust", "cpp", "baseline"}:
                raise RuntimeError("unknown action feedback endpoint language")
            validate_endpoint(
                endpoint, role, metadata["rmw"], record["elements"], record["goal_window"],
                feedback_per_goal, metadata["iterations"], metadata["warmup"],
                "rclcpp" if language == "cpp" else "r2r", instrumentation["enabled"],
                server_stages, status_qos, result_timeout_ms, status_batch_size,
                None if status_paths is None else status_paths[language], status_sync,
                (None if role != "ping" or feedback_delivery_paths is None
                 else feedback_delivery_paths[language]))
    expected = {(elements, window, ping, echo, repetition)
                for elements in sizes for window in windows for ping, echo in pairs
                for repetition in range(metadata["repetitions"])}
    if keys != expected:
        raise RuntimeError("incomplete action feedback language/dimension matrix")
    fields = ["rtt_ns_p50", "rtt_ns_p95", "rtt_ns_p99", "feedback_ns_p50",
              "feedback_ns_p95", "feedback_ns_p99", "roundtrips_per_second",
              "feedback_messages_per_second"]
    expected_rows = set()
    for row in report["summary"]:
        identity = (row["elements"], row["goal_window"], row["ping_language"], row["echo_language"])
        if identity in expected_rows:
            raise RuntimeError("duplicate action feedback summary row")
        expected_rows.add(identity)
        group = [record["ping"] for record in records
                 if (record["elements"], record["goal_window"], record["ping_language"], record["echo_language"]) == identity]
        if len(group) != metadata["repetitions"]:
            raise RuntimeError("missing action feedback repetition")
        for field in fields:
            if row.get(field) != statistics.median(record[field] for record in group):
                raise RuntimeError("action feedback summary median mismatch")
        if server_stages:
            echo_group = [record["echo"] for record in records
                          if (record["elements"], record["goal_window"], record["ping_language"], record["echo_language"]) == identity]
            for percent in [50, 95, 99]:
                field = f"server_stage_ns_p{percent}"
                expected = {
                    stage: statistics.median(record[field][stage] for record in echo_group)
                    for stage in SERVER_STAGES
                }
                if row.get(field) != expected:
                    raise RuntimeError("action feedback server stage summary mismatch")
    actual_rows = {(elements, window, ping, echo)
                   for elements in sizes for window in windows for ping, echo in pairs}
    if expected_rows != actual_rows:
        raise RuntimeError("missing action feedback summary row")
    if "regression_gate" in metadata:
        if instrumentation["enabled"]:
            raise RuntimeError("instrumented action feedback timing cannot pass a regression gate")
        gate = metadata["regression_gate"]
        failures = regression_failures(report["summary"], sizes, windows, gate["maximum_ratio"])
        if gate.get("passed") != (not failures) or gate.get("failures") != failures:
            raise RuntimeError("recorded action feedback gate does not match measurements")
    if "rclcpp_gate" in metadata:
        if instrumentation["enabled"]:
            raise RuntimeError("instrumented action feedback timing cannot pass an rclcpp gate")
        gate = metadata["rclcpp_gate"]
        failures = rclcpp_failures(report["summary"], sizes, windows, gate["maximum_ratio"])
        if gate.get("passed") != (not failures) or gate.get("failures") != failures:
            raise RuntimeError("recorded action feedback rclcpp gate does not match measurements")
    performance_gates = [metadata[name] for name in ("regression_gate", "rclcpp_gate")
                         if name in metadata]
    if feedback_delivery_paths is not None and performance_gates:
        isolation = metadata.get("isolation_gate")
        if (not isinstance(isolation, dict)
                or isolation.get("passed") is not True
                or not qualified_isolation(isolation, DEFAULT_FORBIDDEN_PROCESS_NAMES)):
            raise RuntimeError("native action feedback performance gates require passing isolation")
    if require_performance_pass and (
            not performance_gates or any(gate.get("passed") is not True for gate in performance_gates)
    ):
        raise RuntimeError("recorded passing action feedback performance gates are required")
    if require_current_sources:
        root = Path(__file__).resolve().parents[2]
        mismatches = current_source_mismatches(metadata, root)
        if mismatches:
            raise RuntimeError(f"action feedback source manifest mismatches current tree: {mismatches}")
    print(f"{path}: {len(records)} records and {sum(len(trace['samples_ns']) for trace in traces)} batch samples verified")


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("report", type=Path, nargs="+")
    parser.add_argument("--require-performance-pass", action="store_true")
    parser.add_argument("--require-current-sources", action="store_true")
    args = parser.parse_args()
    for path in args.report:
        verify(path, args.require_performance_pass, args.require_current_sources)


if __name__ == "__main__":
    main()
