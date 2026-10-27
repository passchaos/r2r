#!/usr/bin/env python3
"""Verify archived endpoint records, raw RTT samples and summary medians."""
import argparse
import gzip
import hashlib
import json
from pathlib import Path
import statistics

from transport import regression_failures, validate_endpoint


def verify(path, require_performance_pass=False):
    report = json.loads(path.read_text())
    metadata = report["metadata"]
    workload = metadata.get("workload", "pubsub")
    if workload not in {"pubsub", "service", "action"}:
        raise RuntimeError("unknown transport workload")
    dimension = {"pubsub": "elements", "service": "window", "action": "scenario"}[workload]
    artifact = metadata["samples_artifact"]
    compressed = (path.parent / artifact["file"]).read_bytes()
    if hashlib.sha256(compressed).hexdigest() != artifact["sha256"]:
        raise RuntimeError("sample artifact digest mismatch")
    traces = json.loads(gzip.decompress(compressed))
    records = report["records"]
    if len(traces) != len(records) or artifact["record_count"] != len(records):
        raise RuntimeError("sample/record count mismatch")
    indexes = {trace["record"] for trace in traces}
    if indexes != set(range(len(records))) or len(indexes) != len(traces):
        raise RuntimeError("sample record references are missing or duplicated")
    keys = set()
    for trace in traces:
        record = records[trace["record"]]
        key = (record[dimension], record["ping_language"], record["echo_language"], record["repetition"])
        if key in keys:
            raise RuntimeError("duplicate benchmark record")
        keys.add(key)
        if record["ping_language"] not in {"rust", "cpp", "baseline"} or record["echo_language"] not in {"rust", "cpp", "baseline"}:
            raise RuntimeError("unknown endpoint language")
        for role in ["ping", "echo"]:
            endpoint = dict(record[role])
            if role == "ping":
                endpoint["samples_ns"] = trace["samples_ns"]
                if workload == "action": endpoint["stage_samples_ns"] = trace["stage_samples_ns"]
            language = record[f"{role}_language"]
            validate_endpoint(endpoint, role, metadata["rmw"], record[dimension], metadata["iterations"],
                              metadata["warmup"], "rclcpp" if language == "cpp" else "r2r", dimension)
    sizes = metadata.get({"pubsub": "payload_sizes", "service": "request_windows", "action": "action_scenarios"}[workload],
                         sorted({record[dimension] for record in records}))
    pairs = [("rust", "rust"), ("cpp", "cpp"), ("rust", "cpp"), ("cpp", "rust")]
    if "baseline" in metadata.get("binary_sha256", {}):
        pairs += [("baseline", "baseline"), ("baseline", "cpp"), ("cpp", "baseline")]
    expected_keys = {(size, ping, echo, repetition) for size in sizes for ping, echo in pairs
                     for repetition in range(metadata["repetitions"])}
    if keys != expected_keys:
        raise RuntimeError("incomplete language, payload or repetition matrix")
    for row in report["summary"]:
        group = [record["ping"] for record in records if (record[dimension], record["ping_language"], record["echo_language"]) ==
                 (row[dimension], row["ping_language"], row["echo_language"])]
        if len(group) != metadata["repetitions"]:
            raise RuntimeError("missing repetition")
        fields = ["rtt_ns_p50", "rtt_ns_p95", "rtt_ns_p99", "roundtrips_per_second"]
        if workload == "action": fields.append("acceptance_ns_p50")
        if workload == "action" and row[dimension] == 1: fields.append("cancellation_ns_p50")
        for field in fields:
            if row[field] != statistics.median(record[field] for record in group):
                raise RuntimeError("summary median mismatch")
    expected_rows = {(record[dimension], record["ping_language"], record["echo_language"]) for record in records}
    actual_rows = {(row[dimension], row["ping_language"], row["echo_language"]) for row in report["summary"]}
    if expected_rows != actual_rows or len(actual_rows) != len(report["summary"]):
        raise RuntimeError("summary rows are missing or duplicated")
    if "regression_gate" in metadata:
        gate = metadata["regression_gate"]
        failures = regression_failures(report["summary"], sizes, gate["maximum_ratio"], dimension)
        if gate["passed"] != (not failures) or gate["failures"] != failures:
            raise RuntimeError("recorded regression gate does not match measurements")
    if require_performance_pass:
        if "regression_gate" not in metadata or not metadata["regression_gate"]["passed"]:
            raise RuntimeError("a recorded, passing performance regression gate is required")
    print(f"{path}: {len(records)} records and {sum(len(trace['samples_ns']) for trace in traces)} RTT samples verified")


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("report", type=Path, nargs="+")
    parser.add_argument("--require-performance-pass", action="store_true")
    args = parser.parse_args()
    for path in args.report:
        verify(path, args.require_performance_pass)


if __name__ == "__main__":
    main()
