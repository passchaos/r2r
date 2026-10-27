#!/usr/bin/env python3
"""Verify archived endpoint records, raw RTT samples and summary medians."""
import argparse
import gzip
import hashlib
import json
from pathlib import Path
import statistics

from transport import validate_endpoint


def verify(path):
    report = json.loads(path.read_text())
    metadata = report["metadata"]
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
        key = (record["elements"], record["ping_language"], record["echo_language"], record["repetition"])
        if key in keys:
            raise RuntimeError("duplicate benchmark record")
        keys.add(key)
        if record["ping_language"] not in {"rust", "cpp"} or record["echo_language"] not in {"rust", "cpp"}:
            raise RuntimeError("unknown endpoint language")
        for role in ["ping", "echo"]:
            endpoint = dict(record[role])
            if role == "ping":
                endpoint["samples_ns"] = trace["samples_ns"]
            language = record[f"{role}_language"]
            validate_endpoint(endpoint, role, metadata["rmw"], record["elements"], metadata["iterations"],
                              metadata["warmup"], "r2r" if language == "rust" else "rclcpp")
    sizes = metadata.get("payload_sizes", sorted({record["elements"] for record in records}))
    expected_keys = {(size, ping, echo, repetition) for size in sizes for ping in ["rust", "cpp"]
                     for echo in ["rust", "cpp"] for repetition in range(metadata["repetitions"])}
    if keys != expected_keys:
        raise RuntimeError("incomplete language, payload or repetition matrix")
    for row in report["summary"]:
        group = [record["ping"] for record in records if (record["elements"], record["ping_language"], record["echo_language"]) ==
                 (row["elements"], row["ping_language"], row["echo_language"])]
        if len(group) != metadata["repetitions"]:
            raise RuntimeError("missing repetition")
        for field in ["rtt_ns_p50", "rtt_ns_p95", "rtt_ns_p99", "roundtrips_per_second"]:
            if row[field] != statistics.median(record[field] for record in group):
                raise RuntimeError("summary median mismatch")
    expected_rows = {(record["elements"], record["ping_language"], record["echo_language"]) for record in records}
    actual_rows = {(row["elements"], row["ping_language"], row["echo_language"]) for row in report["summary"]}
    if expected_rows != actual_rows or len(actual_rows) != len(report["summary"]):
        raise RuntimeError("summary rows are missing or duplicated")
    print(f"{path}: {len(records)} records and {sum(len(trace['samples_ns']) for trace in traces)} RTT samples verified")


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("report", type=Path, nargs="+")
    args = parser.parse_args()
    for path in args.report:
        verify(path)


if __name__ == "__main__":
    main()
