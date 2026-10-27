#!/usr/bin/env python3
"""Measure independent ROS ping/echo processes, including cross-language interoperability."""
import argparse
import datetime
import gzip
import hashlib
import json
import math
import os
from pathlib import Path
import platform
import statistics
import subprocess
import tempfile
import uuid
import xml.etree.ElementTree as ET
from action_support.validation import language_pairs, validate_cohorts
from action_support.paired import paired_cohorts


def percentile(samples, percent):
    ordered = sorted(samples)
    return ordered[math.ceil((len(ordered) - 1) * percent / 100)]


def regression_failures(summary, sizes, maximum, dimension="elements"):
    failures = []
    for size in sizes:
        baseline = next(row for row in summary if row[dimension] == size and row["ping_language"] == row["echo_language"] == "baseline")
        candidate = next(row for row in summary if row[dimension] == size and row["ping_language"] == row["echo_language"] == "rust")
        ratios = {field: candidate[field] / baseline[field] for field in ["rtt_ns_p50", "rtt_ns_p95", "rtt_ns_p99"]}
        ratios["inverse_throughput"] = baseline["roundtrips_per_second"] / candidate["roundtrips_per_second"]
        if any(ratio > maximum for ratio in ratios.values()):
            failures.append({dimension: size, "ratios": ratios})
    return failures


def validate_endpoint(result, role, rmw, elements, iterations, warmup, implementation=None, dimension="elements", cohorts=None, instrumented=None):
    if implementation is not None and result["implementation"] != implementation:
        raise RuntimeError("language implementation mismatch")
    if (result["role"], result["rmw"], result[dimension], result["iterations"], result["warmup"]) != (role, rmw, elements, iterations, warmup):
        raise RuntimeError("endpoint identity mismatch")
    total = iterations + warmup
    if result["received"] != total or result["checksum"] != total * (total - 1) // 2:
        raise RuntimeError("message count or sequence checksum mismatch")
    if dimension == "scenario":
        if elements not in {0, 1} or any(result[field] != total for field in ["accepted", "feedback", "results"]) or result["canceled"] != (total if elements == 1 else 0):
            raise RuntimeError("action lifecycle count mismatch")
    if role == "ping":
        samples = result["samples_ns"]
        if len(samples) != iterations or any(not isinstance(value, int) or value <= 0 for value in samples):
            raise RuntimeError("invalid RTT samples")
        for percent in [50, 95, 99]:
            if result[f"rtt_ns_p{percent}"] != percentile(samples, percent):
                raise RuntimeError("RTT percentile mismatch")
        if result["rtt_ns_max"] != max(samples):
            raise RuntimeError("RTT maximum mismatch")
        minimum_elapsed = max(samples) if dimension == "window" and elements > 1 else sum(samples)
        if result["elapsed_ns"] < minimum_elapsed or not math.isfinite(result["roundtrips_per_second"]) or result["roundtrips_per_second"] <= 0:
            raise RuntimeError("invalid throughput result")
        expected_rate = iterations / (result["elapsed_ns"] * 1e-9)
        # C++ JSON output rounds doubles to six significant digits.
        if not math.isclose(result["roundtrips_per_second"], expected_rate, rel_tol=1e-5):
            raise RuntimeError("throughput calculation mismatch")
        if dimension == "scenario":
            stages = result["stage_samples_ns"]
            if set(stages) != {"acceptance", "cancellation"}:
                raise RuntimeError("action stage labels mismatch")
            for stage, count in [("acceptance", iterations), ("cancellation", iterations if elements == 1 else 0)]:
                values = stages[stage]
                if len(values) != count or any(not isinstance(value, int) or value <= 0 for value in values):
                    raise RuntimeError("invalid action stage samples")
                if count and any(value > samples[index] for index, value in enumerate(values)):
                    raise RuntimeError("action stage exceeds lifecycle time")
            if result["acceptance_ns_p50"] != percentile(stages["acceptance"], 50):
                raise RuntimeError("acceptance percentile mismatch")
            expected_cancel = percentile(stages["cancellation"], 50) if elements == 1 else None
            if result["cancellation_ns_p50"] != expected_cancel:
                raise RuntimeError("cancellation percentile mismatch")
    if dimension == "scenario":
        validate_cohorts(result, role, elements, iterations, warmup, cohorts, instrumented)


def pair(ping, echo, cpu_ping, cpu_echo, env, elements, iterations, warmup, prefix, dimension="elements", cohorts=1, instrumented=None):
    parameters = [str(elements), str(iterations), str(warmup), prefix]
    if dimension == "scenario" and cohorts > 1:
        parameters.append(str(cohorts))
    with tempfile.TemporaryDirectory(prefix="r2r-roundtrip-") as directory:
        output = Path(directory) / "echo.json"
        errors = Path(directory) / "echo.log"
        with output.open("w") as out, errors.open("w") as err:
            responder = subprocess.Popen(["taskset", "-c", str(cpu_echo), str(echo), "echo", *parameters], env=env, stdout=out, stderr=err)
            try:
                initiator = subprocess.run(["taskset", "-c", str(cpu_ping), str(ping), "ping", *parameters], env=env, text=True, capture_output=True, timeout=120)
                if initiator.returncode:
                    raise RuntimeError(f"ping failed ({initiator.returncode}): {initiator.stderr}\necho: {errors.read_text()}")
                if responder.wait(timeout=15):
                    raise RuntimeError(f"echo failed: {errors.read_text()}")
                ping_result = json.loads(initiator.stdout)
                echo_result = json.loads(output.read_text())
            finally:
                if responder.poll() is None:
                    responder.terminate()
                    try:
                        responder.wait(timeout=5)
                    except subprocess.TimeoutExpired:
                        responder.kill()
                        responder.wait()
        validate_endpoint(ping_result, "ping", env["RMW_IMPLEMENTATION"], elements, iterations*cohorts, warmup*cohorts, dimension=dimension, cohorts=cohorts, instrumented=instrumented)
        validate_endpoint(echo_result, "echo", env["RMW_IMPLEMENTATION"], elements, iterations*cohorts, warmup*cohorts, dimension=dimension, cohorts=cohorts, instrumented=instrumented)
        return ping_result, echo_result


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--rust", type=Path, required=True)
    parser.add_argument("--workload", choices=["pubsub", "service", "action"], default="pubsub")
    parser.add_argument("--baseline-rust", type=Path)
    parser.add_argument("--cpp", type=Path, required=True)
    parser.add_argument("--cpu-ping", type=int, required=True)
    parser.add_argument("--cpu-echo", type=int, required=True)
    parser.add_argument("--domain-id", type=int, default=102)
    parser.add_argument("--rmw", default="rmw_cyclonedds_cpp")
    parser.add_argument("--sizes", default="0,32,4096")
    parser.add_argument("--windows", default="1,16", help="service request concurrency windows")
    parser.add_argument("--scenarios", default="0,1", help="action scenarios: 0 success, 1 cancellation")
    parser.add_argument("--action-cohorts", type=int, default=1, help="fresh action endpoint cohorts, each with the requested iterations/warmup")
    parser.add_argument("--action-stability", action="store_true", help="focused Rust baseline/candidate short-cohort regression gate; full interoperability is a separate matrix")
    parser.add_argument("--probe", type=Path, help="optional Linux/glibc allocation probe; counts are separate from latency evidence")
    parser.add_argument("--iterations", type=int, default=5000)
    parser.add_argument("--warmup", type=int, default=100)
    parser.add_argument("--repetitions", type=int, default=5)
    parser.add_argument("--output", type=Path, required=True)
    parser.add_argument("--build-manifest", type=Path)
    parser.add_argument("--max-regression", type=float, help="Maximum candidate/baseline ratio for same-language RTT percentiles and inverse throughput")
    args = parser.parse_args()
    if not 1 <= args.action_cohorts <= 1024 or (args.action_cohorts != 1 and args.workload != "action"):
        parser.error("cohorts require an action workload and a count from 1 to 1024")
    if args.probe and (args.workload != "action" or args.max_regression is not None):
        parser.error("instrumented action runs cannot enforce latency performance gates")
    if args.action_stability and (args.workload != "action" or args.baseline_rust is None or args.max_regression is None
                                 or args.action_cohorts < 2 or args.iterations * args.action_cohorts < 1000
                                 or args.iterations + args.warmup > 15 or args.repetitions < 5):
        parser.error("action stability needs a baseline/gate, at most 15 goals per cohort, at least 1000 measured goals and five repetitions")
    if args.action_stability and args.domain_id > 231:
        parser.error("action stability requires two adjacent available DDS domains")
    allowed = os.sched_getaffinity(0)
    if args.cpu_ping == args.cpu_echo or not {args.cpu_ping, args.cpu_echo}.issubset(allowed):
        parser.error("select two distinct available CPUs")
    if not 0 <= args.domain_id <= 232 or args.iterations <= 0 or args.warmup < 0 or args.repetitions < 1 or args.iterations + args.warmup > 2**32 - 1:
        parser.error("invalid run dimensions or DDS domain")
    dimension = {"pubsub": "elements", "service": "window", "action": "scenario"}[args.workload]
    dimensions = {"pubsub": args.sizes, "service": args.windows, "action": args.scenarios}[args.workload]
    sizes = [int(size) for size in dimensions.split(",")]
    lower, upper = {"pubsub": (0, 1_000_000), "service": (1, 1024), "action": (0, 1)}[args.workload]
    if args.workload == "action" and (args.iterations + args.warmup) * args.action_cohorts > 2**31 - 4:
        parser.error("action sequence count exceeds the goal representation")
    if not sizes or len(set(sizes)) != len(sizes) or any(size < lower or size > upper for size in sizes):
        parser.error("invalid or duplicate workload dimensions")
    if args.max_regression is not None and (args.baseline_rust is None or not math.isfinite(args.max_regression) or args.max_regression < 1):
        parser.error("a regression limit needs a baseline and a finite ratio at least one")
    binaries = {"rust": args.rust.resolve(), "cpp": args.cpp.resolve()}
    if args.baseline_rust:
        binaries["baseline"] = args.baseline_rust.resolve()
    env = os.environ.copy()
    env.update(RMW_IMPLEMENTATION=args.rmw, ROS_DOMAIN_ID=str(args.domain_id), ROS_LOCALHOST_ONLY="1")
    env.pop("CYCLONEDDS_URI", None)
    env.pop("FASTRTPS_DEFAULT_PROFILES_FILE", None)
    env.pop("FASTDDS_DEFAULT_PROFILES_FILE", None)
    if args.probe:
        env["LD_PRELOAD"] = str(args.probe.resolve()) + (":" + env["LD_PRELOAD"] if env.get("LD_PRELOAD") else "")
    metadata = {
        "started_utc": datetime.datetime.now(datetime.timezone.utc).isoformat(),
        "cpu_ping": args.cpu_ping, "cpu_echo": args.cpu_echo, "domain_id": args.domain_id,
        "rmw": args.rmw, "ros_distro": env["ROS_DISTRO"], "platform": platform.platform(),
        "cpu_model": next(line.split(":", 1)[1].strip() for line in Path("/proc/cpuinfo").read_text().splitlines() if line.startswith("model name")),
        "git_head": subprocess.check_output(["git", "rev-parse", "HEAD"], text=True).strip(),
        "working_tree_dirty": bool(subprocess.check_output(["git", "status", "--porcelain"], text=True).strip()),
        "rustc": subprocess.check_output(["rustc", "--version"], text=True).strip(),
        "cxx": subprocess.check_output(["c++", "--version"], text=True).splitlines()[0],
        "binary_sha256": {label: hashlib.sha256(path.read_bytes()).hexdigest() for label, path in binaries.items()},
        "qos": "reliable, volatile, KeepLast(16)", "transport": "independent processes, localhost DDS, intra-process disabled",
        "scheduling": "single-threaded executor/spin loop; 100 ms blocking wait cap; one message in flight",
        "rtt_boundary": "before request publish through response retrieval, before initiator payload validation",
        "echo_work": "complete sequence/layout/payload validation then normal typed publish",
        "throughput_boundary": "all measured roundtrips including initiator validation, excludes warmup/discovery/teardown",
        "iterations": args.iterations*args.action_cohorts, "warmup": args.warmup*args.action_cohorts, "repetitions": args.repetitions,
        "workload": args.workload,
        {"pubsub": "payload_sizes", "service": "request_windows", "action": "action_scenarios"}[args.workload]: sizes,
        "source_sha256": {str(path): hashlib.sha256(path.read_bytes()).hexdigest() for path in
                          [Path("r2r/benches") / {"pubsub": "pubsub_roundtrip.rs", "service": "service_roundtrip.rs", "action": "action_roundtrip.rs"}[args.workload],
                           Path("tools/bench_ros") / {"pubsub": "roundtrip.cpp", "service": "service.cpp", "action": "action.cpp"}[args.workload], Path("tools/bench_ros/transport.py")]},
    }
    if args.workload == "service":
        metadata.update(qos="ServicesQoS: reliable, volatile, KeepLast(max(16, window))",
                        scheduling="single-threaded executor/spin loop; 100 ms wait cap; bounded request window; responses observed in submission order",
                        rtt_boundary="before async service request through response retrieval, before initiator sum validation",
                        echo_work="validate every AddTwoInts sequence and operand, then return exact sum",
                        throughput_boundary="measured completed service requests including validation; excludes discovery/warmup/completion protocol",
                        completion_protocol="-1 stop request and response, then -2 confirmation; all measured responses complete first")
    if args.workload == "action":
        metadata.update(action_cohorts=args.action_cohorts, iterations_per_cohort=args.iterations, warmup_per_cohort=args.warmup,
                        cohort_boundary="new action/ready endpoints for each cohort; shared process/node/context; two rejected controls after all results; setup/discovery/warmup/teardown excluded from aggregated elapsed time",
                        instrumentation={"enabled":bool(args.probe), "latency_evidence":not bool(args.probe)})
        if args.probe:
            metadata["instrumentation"].update(probe_sha256=hashlib.sha256(args.probe.read_bytes()).hexdigest(),
                scope="allocation entry points across all process threads in measured windows; CPU/context switches from getrusage; Rust future wakes only; max RSS is process-lifetime high water")
        metadata.update(qos="goal/result/cancel/feedback: reliable volatile KeepLast(16); status native defaults; result retention 10 s",
                        scheduling="single-threaded executor/spin loop; 100 ms wait cap; one goal in flight",
                        rtt_boundary="before goal submission through acceptance/result observer setup, execution-start readiness service, exactly one validated feedback, feedback acknowledgement or cancellation, and terminal result retrieval",
                        acceptance_boundary="before submission through accepted goal retrieval and sending the result request; no execution-start handshake",
                        cancellation_boundary="before cancel request through accepted cancel response retrieval",
                        auxiliary_protocol="AddTwoInts /ready handshake phase 0 confirms client observation readiness; success phase 1 confirms feedback consumption before completing the result; cancellation instead confirms feedback consumption via the cancel request",
                        echo_work="defer execution until readiness confirmation, validate order, publish exact three-element feedback/result; scenario 1 accepts cancellation before Canceled result",
                        throughput_boundary="complete action lifecycle including validation; excludes discovery/warmup/control rejection protocol",
                        completion_protocol="control goals -1 and -2 rejected after all measured terminal results complete",
                        scenario_labels={"0": "success", "1": "cancel"})
    if args.build_manifest:
        metadata["build_manifest"] = json.loads(args.build_manifest.read_text())
    if args.action_stability:
        metadata["matrix_scope"] = "action_stability"
        metadata["cohort_interleaving"] = "alternate baseline/candidate per cohort; both endpoint pairs resident; inactive pair has no action/ready endpoints; render after all windows"
        metadata["pair_domain_ids"] = {"baseline":args.domain_id,"rust":args.domain_id+1}
    prefix = Path("/opt/ros") / env["ROS_DISTRO"] / "share"
    packages = ["rclcpp", "rcl", "rmw", "example_interfaces" if args.workload in {"service", "action"} else "std_msgs", args.rmw]
    if args.workload == "action": packages += ["rclcpp_action", "rcl_action"]
    metadata["ros_packages"] = {name: ET.parse(prefix / name / "package.xml").findtext("version") for name in packages}
    records, traces = [], []
    pairs = language_pairs(args.baseline_rust is not None, metadata.get("matrix_scope", "full"))
    for elements in sizes:
        for repetition in range(args.repetitions):
            paired = paired_cohorts(binaries["baseline"],binaries["rust"],args.cpu_ping,args.cpu_echo,env,elements,args.iterations,args.warmup,args.action_cohorts,repetition) if args.action_stability else None
            for ping_label, echo_label in pairs if repetition % 2 == 0 else reversed(pairs):
                run_prefix = "/r2r_roundtrip_" + uuid.uuid4().hex
                ping, echo = paired[ping_label] if paired is not None else pair(binaries[ping_label], binaries[echo_label], args.cpu_ping, args.cpu_echo, env, elements, args.iterations, args.warmup, run_prefix, dimension, args.action_cohorts, bool(args.probe))
                validate_endpoint(ping, "ping", args.rmw, elements, args.iterations*args.action_cohorts, args.warmup*args.action_cohorts,
                                  "rclcpp" if ping_label == "cpp" else "r2r", dimension, args.action_cohorts, bool(args.probe))
                validate_endpoint(echo, "echo", args.rmw, elements, args.iterations*args.action_cohorts, args.warmup*args.action_cohorts,
                                  "rclcpp" if echo_label == "cpp" else "r2r", dimension, args.action_cohorts, bool(args.probe))
                samples = ping.pop("samples_ns")
                stages = ping.pop("stage_samples_ns", None)
                record = {"ping_language": ping_label, "echo_language": echo_label, "repetition": repetition, dimension: elements, "ping": ping, "echo": echo}
                trace = {"record": len(records), "samples_ns": samples}
                if stages is not None: trace["stage_samples_ns"] = stages
                traces.append(trace)
                records.append(record)
                print(f"{ping_label}/{echo_label} {dimension}={elements} repetition={repetition + 1}/{args.repetitions}: p50={ping['rtt_ns_p50']} ns p99={ping['rtt_ns_p99']} ns", flush=True)
    summary = []
    for elements in sizes:
        for ping_label, echo_label in pairs:
            group = [record["ping"] for record in records if record[dimension] == elements and record["ping_language"] == ping_label and record["echo_language"] == echo_label]
            fields = ["rtt_ns_p50", "rtt_ns_p95", "rtt_ns_p99", "roundtrips_per_second"]
            if args.workload == "action": fields.append("acceptance_ns_p50")
            if args.workload == "action" and elements == 1: fields.append("cancellation_ns_p50")
            summary.append({dimension: elements, "ping_language": ping_label, "echo_language": echo_label,
                            **{key: statistics.median(record[key] for record in group) for key in fields}})
    args.output.parent.mkdir(parents=True, exist_ok=True)
    trace_path = args.output.with_suffix(".samples.json.gz")
    compressed = gzip.compress(json.dumps(traces, separators=(",", ":")).encode(), mtime=0)
    trace_path.write_bytes(compressed)
    metadata["samples_artifact"] = {"file": trace_path.name, "sha256": hashlib.sha256(compressed).hexdigest(), "record_count": len(traces)}
    metadata["completed_utc"] = datetime.datetime.now(datetime.timezone.utc).isoformat()
    regressions = regression_failures(summary, sizes, args.max_regression, dimension) if args.max_regression is not None else []
    if args.max_regression is not None:
        metadata["regression_gate"] = {"maximum_ratio": args.max_regression,
                                       "passed": not regressions, "failures": regressions}
    args.output.write_text(json.dumps({"metadata": metadata, "records": records, "summary": summary}, indent=2) + "\n")
    print(json.dumps(summary, indent=2))
    if args.max_regression is not None:
        if regressions:
            raise RuntimeError(f"transport regression gate failed: {regressions}")


if __name__ == "__main__":
    main()
