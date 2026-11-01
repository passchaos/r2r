#!/usr/bin/env python3
"""Summarize per-process peak RSS from independent-process pipeline reports."""

import argparse
import json
import statistics


CASES = [(0, 1, 1), (4096, 16, 1), (4096, 8, 4)]
MAX_RATIO = 2.0


def summarize(path):
    report = json.load(path)
    records = report["records"]
    if len(records) != 12:
        raise RuntimeError(f"{path}: expected twelve pipeline records")
    summary = {}
    for case in CASES:
        for role in ["ping", "echo"]:
            rust, cpp = [], []
            for record in records:
                if (record["elements"], record["window"], record["fanout"]) != case:
                    continue
                language = record["ping_language"] if role == "ping" else record["echo_language"]
                rss = record[role].get("process_max_rss_kib")
                if type(rss) is not int or rss <= 0:
                    raise RuntimeError(f"{path}: missing process RSS")
                (rust if language == "rust" else cpp).append(rss)
            if not rust or not cpp:
                raise RuntimeError(f"{path}: incomplete language matrix for {case}/{role}")
            rust_rss = statistics.median(rust)
            cpp_rss = statistics.median(cpp)
            ratio = rust_rss / cpp_rss
            if ratio < 1.0 / MAX_RATIO or ratio > MAX_RATIO:
                raise RuntimeError(f"{path}: implausible RSS ratio {ratio}")
            summary[f"{case[0]}_{case[1]}_{case[2]}_{role}"] = {
                "rust_median_rss_kib": rust_rss,
                "cpp_median_rss_kib": cpp_rss,
                "rust_to_cpp_ratio": ratio,
            }
        same_language = [
            record for record in records
            if (record["elements"], record["window"], record["fanout"]) == case
            and record["ping_language"] == record["echo_language"]
        ]
        rust_cpu = next(record["ping"]["pair_user_cpu_ns"] for record in same_language
                        if record["ping_language"] == "rust")
        cpp_cpu = next(record["ping"]["pair_user_cpu_ns"] for record in same_language
                       if record["ping_language"] == "cpp")
        if rust_cpu <= 0 or cpp_cpu <= 0:
            raise RuntimeError(f"{path}: missing pair CPU")
        cpu_ratio = rust_cpu / cpp_cpu
        if cpu_ratio < 0.25 or cpu_ratio > 4.0:
            raise RuntimeError(f"{path}: implausible pair CPU ratio {cpu_ratio}")
        summary[f"{case[0]}_{case[1]}_{case[2]}_pair_cpu"] = {
            "rust_user_cpu_ns": rust_cpu,
            "cpp_user_cpu_ns": cpp_cpu,
            "rust_to_cpp_ratio": cpu_ratio,
        }
    return {
        "rmw": report["metadata"]["rmw"],
        "receive_paths": report["metadata"]["receive_paths"],
        "summary": summary,
    }


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--cyclonedds", type=argparse.FileType("r"), required=True)
    parser.add_argument("--fastdds", type=argparse.FileType("r"), required=True)
    parser.add_argument("--output", type=argparse.FileType("w"), required=True)
    args = parser.parse_args()
    result = {
        "schema_version": 1,
        "timing_qualified": False,
        "reports": {
            "cyclonedds": summarize(args.cyclonedds),
            "fastdds": summarize(args.fastdds),
        },
    }
    json.dump(result, args.output, indent=2)
    args.output.write("\n")
    print(json.dumps({rmw: report["summary"] for rmw, report in result["reports"].items()}, indent=2))


if __name__ == "__main__":
    main()
