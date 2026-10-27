#!/usr/bin/env python3
"""Recheck codec comparison workload identities, medians and regression gates."""
import argparse
import json
from pathlib import Path
import statistics

from run import regression_failures, validate_result


def verify(path, require_performance_pass=False):
    report = json.loads(path.read_text())
    records, metadata = report["records"], report["metadata"]
    labels = set(metadata["binary_sha256"])
    sizes = metadata.get("payload_sizes", sorted({row["elements"] for row in report["summary"]}))
    modes = metadata.get("modes", list(dict.fromkeys(row["mode"] for row in report["summary"])))
    repetitions = metadata.get("repetitions", max(row["repetition"] for row in records) + 1)
    expected = {(size, mode, label, repetition) for size in sizes for mode in modes for label in labels for repetition in range(repetitions)}
    actual = {(row["elements"], row["mode"], row["label"], row["repetition"]) for row in records}
    if actual != expected or len(actual) != len(records):
        raise RuntimeError("incomplete or duplicate codec matrix")
    for size in sizes:
        size_records = [row for row in records if row["elements"] == size]
        fixture = size_records[0]
        for mode in modes:
            group = [row for row in size_records if row["mode"] == mode]
            reference = group[0]
            for row in group:
                validate_result(row, metadata["rmw"], mode, size, reference["iterations"], fixture["wire_bytes"], fixture["wire_hash"])
                if row["checksum"] != reference["checksum"] or row["implementation"] != ("rclcpp" if row["label"] == "rclcpp" else "r2r"):
                    raise RuntimeError("codec result identity or checksum mismatch")
    rows = {(row["elements"], row["mode"]) for row in report["summary"]}
    if rows != {(size, mode) for size in sizes for mode in modes} or len(rows) != len(report["summary"]):
        raise RuntimeError("incomplete or duplicate codec summary")
    for row in report["summary"]:
        if set(row["median_ns_per_op"]) != labels or set(row["relative_to_rclcpp"]) != labels:
            raise RuntimeError("summary label mismatch")
        for label in labels:
            median = statistics.median(record["ns_per_op"] for record in records if
                                       (record["elements"], record["mode"], record["label"]) == (row["elements"], row["mode"], label))
            if row["median_ns_per_op"][label] != median or row["relative_to_rclcpp"][label] != median / row["median_ns_per_op"]["rclcpp"]:
                raise RuntimeError("codec summary does not match raw records")
    if "regression_gate" in metadata:
        gate = metadata["regression_gate"]
        failures = regression_failures(report["summary"], gate["maximum_ratio"])
        if gate["passed"] != (not failures) or gate["failures"] != failures:
            raise RuntimeError("codec regression gate does not match measurements")
    if require_performance_pass and ("regression_gate" not in metadata or not metadata["regression_gate"]["passed"]):
        raise RuntimeError("a recorded passing codec regression gate is required")
    print(f"{path}: {len(records)} codec records verified")


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("report", type=Path, nargs="+")
    parser.add_argument("--require-performance-pass", action="store_true")
    args = parser.parse_args()
    for path in args.report:
        verify(path, args.require_performance_pass)


if __name__ == "__main__":
    main()
