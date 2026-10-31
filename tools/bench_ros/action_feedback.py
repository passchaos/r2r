#!/usr/bin/env python3
"""Measure concurrent, large-payload action feedback across independent Rust/C++ processes."""
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

from action_support.validation import language_pairs, validate_probe
from pubsub_pipeline import DEFAULT_FORBIDDEN_PROCESS_NAMES, IsolationMonitor, wait_for_isolation

SERVER_STAGES = ("accept", "start_wait", "execute", "feedback_publish", "feedback_wait", "terminal")


def percentile(samples, percent):
    ordered = sorted(samples)
    return ordered[math.ceil((len(ordered) - 1) * percent / 100)]


def matrix_pairs(baseline, scope):
    if scope == "full":
        return language_pairs(baseline)
    if scope == "regression" and baseline:
        return [("baseline", "baseline"), ("rust", "rust")]
    if scope == "server":
        return [("cpp", "cpp"), ("cpp", "rust")]
    raise RuntimeError("regression scope requires an independent baseline")


def validate_endpoint(result, role, rmw, elements, goal_window, feedback_per_goal,
                      iterations, warmup, implementation=None, instrumented=None,
                      server_stages=False, status_qos=None, result_timeout_ms=None,
                      status_batch_size=None, status_path=None, status_sync=None,
                      feedback_delivery=None):
    if implementation is not None and result.get("implementation") != implementation:
        raise RuntimeError("language implementation mismatch")
    identity = (result.get("role"), result.get("rmw"), result.get("elements"),
                result.get("goal_window"), result.get("feedback_per_goal"),
                result.get("iterations"), result.get("warmup"))
    expected = (role, rmw, elements, goal_window, feedback_per_goal, iterations, warmup)
    if identity != expected:
        raise RuntimeError("action feedback endpoint identity mismatch")
    if status_path is None:
        if "status_path" in result:
            raise RuntimeError("undeclared action feedback status path")
    elif result.get("status_path") != status_path:
        raise RuntimeError("action feedback status path mismatch")
    if status_sync is None:
        if "status_sync" in result:
            raise RuntimeError("undeclared action feedback status sync")
    elif result.get("status_sync") != status_sync:
        raise RuntimeError("action feedback status sync mismatch")
    if role == "ping":
        if feedback_delivery is None:
            if "feedback_delivery" in result:
                raise RuntimeError("undeclared action feedback delivery path")
        elif result.get("feedback_delivery") != feedback_delivery:
            raise RuntimeError("action feedback delivery path mismatch")
    elif "feedback_delivery" in result:
        raise RuntimeError("feedback delivery path is only valid for the client endpoint")
    if status_qos is None:
        if "status_qos" in result:
            raise RuntimeError("undeclared action feedback status QoS")
    elif result.get("status_qos") != status_qos:
        raise RuntimeError("action feedback status QoS mismatch")
    if result_timeout_ms is None:
        if "result_timeout_ms" in result:
            raise RuntimeError("undeclared action feedback result timeout")
    elif result.get("result_timeout_ms") != result_timeout_ms:
        raise RuntimeError("action feedback result timeout mismatch")
    if status_batch_size is None:
        if "status_batch_size" in result:
            raise RuntimeError("undeclared action feedback status batch size")
    elif result.get("status_batch_size") != status_batch_size:
        raise RuntimeError("action feedback status batch size mismatch")
    total = iterations + warmup
    goals = total * goal_window
    if (result.get("received") != total
            or result.get("batch_checksum") != total * (total - 1) // 2
            or result.get("goal_checksum") != goals * (goals + 1) // 2
            or result.get("accepted") != goals
            or result.get("feedback") != goals * feedback_per_goal
            or result.get("results") != goals):
        raise RuntimeError("action feedback count or checksum mismatch")
    probe = result.get("probe")
    if instrumented is not None and (probe is not None) != instrumented:
        raise RuntimeError("action feedback instrumentation mode mismatch")
    if probe is not None:
        validate_probe(probe, result["implementation"])
    if role != "ping":
        stage_keys = {"server_stage_ns_p50", "server_stage_ns_p95", "server_stage_ns_p99",
                      "server_stage_samples_ns"}
        if not server_stages:
            if stage_keys & result.keys():
                raise RuntimeError("undeclared action feedback server stages")
            return
        if not stage_keys <= result.keys():
            raise RuntimeError("missing action feedback server stages")
        samples = result["server_stage_samples_ns"]
        if set(samples) != set(SERVER_STAGES):
            raise RuntimeError("action feedback server stage labels mismatch")
        for stage in SERVER_STAGES:
            values = samples[stage]
            if (not isinstance(values, list) or len(values) != iterations
                    or any(type(value) is not int or value <= 0 for value in values)):
                raise RuntimeError("invalid action feedback server stage samples")
            for percent in [50, 95, 99]:
                percentiles = result[f"server_stage_ns_p{percent}"]
                if set(percentiles) != set(SERVER_STAGES) or percentiles[stage] != percentile(values, percent):
                    raise RuntimeError("action feedback server stage percentile mismatch")
        return
    samples = result.get("samples_ns")
    feedback_samples = result.get("feedback_samples_ns")
    if (not isinstance(samples, list) or len(samples) != iterations
            or not isinstance(feedback_samples, list) or len(feedback_samples) != iterations
            or any(type(value) is not int or value <= 0 for value in samples + feedback_samples)
            or any(feedback > lifecycle for feedback, lifecycle in zip(feedback_samples, samples))):
        raise RuntimeError("invalid action feedback timing samples")
    for percent in [50, 95, 99]:
        if result.get(f"rtt_ns_p{percent}") != percentile(samples, percent):
            raise RuntimeError("action feedback RTT percentile mismatch")
        if result.get(f"feedback_ns_p{percent}") != percentile(feedback_samples, percent):
            raise RuntimeError("action feedback stage percentile mismatch")
    if result.get("rtt_ns_max") != max(samples):
        raise RuntimeError("action feedback RTT maximum mismatch")
    elapsed = result.get("elapsed_ns")
    if type(elapsed) is not int or elapsed < sum(samples):
        raise RuntimeError("invalid action feedback elapsed time")
    expected_batches = iterations / (elapsed * 1e-9)
    expected_feedback = iterations * goal_window * feedback_per_goal / (elapsed * 1e-9)
    for field, expected_rate in [("roundtrips_per_second", expected_batches),
                                 ("feedback_messages_per_second", expected_feedback)]:
        value = result.get(field)
        if not isinstance(value, (int, float)) or not math.isfinite(value) or value <= 0 or not math.isclose(value, expected_rate, rel_tol=1e-9):
            raise RuntimeError(f"invalid {field}")


def regression_failures(summary, sizes, windows, maximum):
    failures = []
    for elements in sizes:
        for goal_window in windows:
            baseline = next(row for row in summary if row["elements"] == elements
                            and row["goal_window"] == goal_window
                            and row["ping_language"] == row["echo_language"] == "baseline")
            candidate = next(row for row in summary if row["elements"] == elements
                             and row["goal_window"] == goal_window
                             and row["ping_language"] == row["echo_language"] == "rust")
            ratios = {field: candidate[field] / baseline[field]
                      for field in ["rtt_ns_p50", "rtt_ns_p95", "rtt_ns_p99",
                                    "feedback_ns_p50", "feedback_ns_p95", "feedback_ns_p99"]}
            ratios["inverse_throughput"] = baseline["feedback_messages_per_second"] / candidate["feedback_messages_per_second"]
            if any(ratio > maximum for ratio in ratios.values()):
                failures.append({"elements": elements, "goal_window": goal_window, "ratios": ratios})
    return failures


def rclcpp_failures(summary, sizes, windows, maximum):
    failures = []
    for elements in sizes:
        for goal_window in windows:
            rust = next(row for row in summary if row["elements"] == elements
                        and row["goal_window"] == goal_window
                        and row["ping_language"] == row["echo_language"] == "rust")
            cpp = next(row for row in summary if row["elements"] == elements
                       and row["goal_window"] == goal_window
                       and row["ping_language"] == row["echo_language"] == "cpp")
            ratios = {field: rust[field] / cpp[field]
                      for field in ["rtt_ns_p50", "rtt_ns_p95", "rtt_ns_p99",
                                    "feedback_ns_p50", "feedback_ns_p95", "feedback_ns_p99"]}
            ratios["inverse_throughput"] = (
                cpp["feedback_messages_per_second"] / rust["feedback_messages_per_second"]
            )
            if any(ratio > maximum for ratio in ratios.values()):
                failures.append({"elements": elements, "goal_window": goal_window,
                                 "ratios": ratios})
    return failures


def pair(ping, echo, cpu_ping, cpu_echo, env, elements, goal_window, feedback_per_goal,
         iterations, warmup, prefix, instrumented, server_stages=True, status_qos="default",
         ping_status_path="per_goal", echo_status_path="per_goal", status_sync="required",
         ping_feedback_delivery="owned", echo_feedback_delivery="owned"):
    parameters = [str(elements), str(goal_window), str(feedback_per_goal),
                  str(iterations), str(warmup), prefix]
    with tempfile.TemporaryDirectory(prefix="r2r-action-feedback-") as directory:
        output = Path(directory) / "echo.json"
        errors = Path(directory) / "echo.log"
        ping_env = env.copy()
        echo_env = env.copy()
        ping_env["R2R_ACTION_FEEDBACK_DELIVERY"] = (
            ping_feedback_delivery if ping_feedback_delivery in {"owned", "native"} else "owned"
        )
        echo_env["R2R_ACTION_FEEDBACK_DELIVERY"] = (
            echo_feedback_delivery if echo_feedback_delivery in {"owned", "native"} else "owned"
        )
        with output.open("w") as out, errors.open("w") as err:
            responder = subprocess.Popen(
                ["taskset", "-c", str(cpu_echo), str(echo), "echo", *parameters],
                env=echo_env, stdout=out, stderr=err)
            try:
                initiator = subprocess.run(
                    ["taskset", "-c", str(cpu_ping), str(ping), "ping", *parameters],
                    env=ping_env, text=True, capture_output=True, timeout=300)
                if initiator.returncode:
                    raise RuntimeError(
                        f"ping failed ({initiator.returncode}): {initiator.stderr}\n"
                        f"echo: {errors.read_text()}")
                if responder.wait(timeout=30):
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
    validate_endpoint(ping_result, "ping", env["RMW_IMPLEMENTATION"], elements,
                      goal_window, feedback_per_goal, iterations, warmup,
                      instrumented=instrumented, server_stages=server_stages,
                      status_qos=status_qos,
                      result_timeout_ms=int(env["R2R_ACTION_FEEDBACK_RESULT_TIMEOUT_MS"]),
                      status_batch_size=int(env["R2R_ACTION_FEEDBACK_STATUS_BATCH_SIZE"]),
                      status_path=ping_status_path, status_sync=status_sync,
                      feedback_delivery=ping_feedback_delivery)
    validate_endpoint(echo_result, "echo", env["RMW_IMPLEMENTATION"], elements,
                      goal_window, feedback_per_goal, iterations, warmup,
                      instrumented=instrumented, server_stages=server_stages,
                      status_qos=status_qos,
                      result_timeout_ms=int(env["R2R_ACTION_FEEDBACK_RESULT_TIMEOUT_MS"]),
                      status_batch_size=int(env["R2R_ACTION_FEEDBACK_STATUS_BATCH_SIZE"]),
                      status_path=echo_status_path, status_sync=status_sync)
    return ping_result, echo_result


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--rust", type=Path, required=True)
    parser.add_argument("--baseline-rust", type=Path)
    parser.add_argument("--cpp", type=Path, required=True)
    parser.add_argument("--cpu-ping", type=int, required=True)
    parser.add_argument("--cpu-echo", type=int, required=True)
    parser.add_argument("--domain-id", type=int, default=150)
    parser.add_argument("--rmw", default="rmw_cyclonedds_cpp")
    parser.add_argument("--sizes", default="0,4096")
    parser.add_argument("--goal-windows", default="1,8")
    parser.add_argument("--feedback-per-goal", type=int, default=4)
    parser.add_argument("--iterations", type=int, default=100)
    parser.add_argument("--warmup", type=int, default=10)
    parser.add_argument("--repetitions", type=int, default=5)
    parser.add_argument("--probe", type=Path)
    parser.add_argument("--build-manifest", type=Path)
    parser.add_argument("--max-regression", type=float)
    parser.add_argument("--max-rclcpp-ratio", type=float)
    parser.add_argument("--matrix-scope", choices=["full", "regression", "server"], default="full")
    parser.add_argument("--status-qos", choices=["default", "reliable_volatile", "best_effort_volatile"], default="default")
    parser.add_argument("--result-timeout-ms", type=int, default=10_000)
    parser.add_argument("--status-batch-size", type=int)
    parser.add_argument("--rust-status-path", choices=["per_goal", "batched"], default="batched")
    parser.add_argument("--baseline-status-path", choices=["per_goal", "batched"], default="per_goal")
    parser.add_argument("--status-sync", choices=["required", "none"], default="required")
    parser.add_argument("--rust-feedback-delivery", choices=["owned", "native"], default="native")
    parser.add_argument("--baseline-feedback-delivery", choices=["owned", "native"], default="owned")
    parser.add_argument("--isolation-samples", type=int, default=0,
                        help="required consecutive idle samples before measurement (zero disables)")
    parser.add_argument("--isolation-interval", type=float, default=20.0)
    parser.add_argument("--isolation-timeout", type=float, default=300.0)
    parser.add_argument("--isolation-max-load1", type=float, default=2.0)
    parser.add_argument("--isolation-monitor-interval", type=float, default=0.25)
    parser.add_argument("--isolation-forbid-comm",
                        default=",".join(DEFAULT_FORBIDDEN_PROCESS_NAMES))
    parser.add_argument("--output", type=Path, required=True)
    args = parser.parse_args()
    sizes = [int(value) for value in args.sizes.split(",")]
    windows = [int(value) for value in args.goal_windows.split(",")]
    if args.status_batch_size is None:
        if len(windows) != 1:
            parser.error("multiple goal windows require an explicit status batch size")
        args.status_batch_size = windows[0]
    if (not sizes or len(set(sizes)) != len(sizes) or any(not 0 <= value <= 1_000_000 for value in sizes)
            or not windows or len(set(windows)) != len(windows) or any(not 1 <= value <= 64 for value in windows)
            or not 1 <= args.feedback_per_goal <= 256
            or max(windows) * args.feedback_per_goal > 4096
            or args.iterations <= 0 or args.warmup < 0 or args.repetitions <= 0
            or not 1 <= args.result_timeout_ms <= 600_000
            or not 1 <= args.status_batch_size <= min(windows)
            or (args.iterations + args.warmup) * max(windows) > 2**31 - 2):
        parser.error("invalid action feedback dimensions")
    if not 0 <= args.domain_id <= 232:
        parser.error("invalid DDS domain")
    allowed = os.sched_getaffinity(0)
    if args.cpu_ping == args.cpu_echo or not {args.cpu_ping, args.cpu_echo}.issubset(allowed):
        parser.error("select two distinct available CPUs")
    if args.max_regression is not None and (args.baseline_rust is None
            or not math.isfinite(args.max_regression) or args.max_regression < 1):
        parser.error("a regression limit needs an independent baseline and a finite ratio at least one")
    if args.matrix_scope == "regression" and args.baseline_rust is None:
        parser.error("regression matrix scope requires an independent baseline")
    if args.matrix_scope == "server" and args.max_regression is not None:
        parser.error("server diagnostic scope does not define a regression gate")
    if args.max_rclcpp_ratio is not None and (
            args.matrix_scope != "full" or not math.isfinite(args.max_rclcpp_ratio)
            or args.max_rclcpp_ratio <= 0):
        parser.error("an rclcpp limit needs full scope and a finite positive ratio")
    if args.probe and args.max_regression is not None:
        parser.error("instrumented runs cannot enforce latency regression gates")
    if args.probe and args.max_rclcpp_ratio is not None:
        parser.error("instrumented runs cannot enforce an rclcpp latency gate")
    forbidden_names = tuple(dict.fromkeys(
        name.strip() for name in args.isolation_forbid_comm.split(",") if name.strip()))
    if (args.isolation_samples < 0 or not math.isfinite(args.isolation_interval)
            or args.isolation_interval <= 0 or not math.isfinite(args.isolation_timeout)
            or args.isolation_timeout <= 0 or not math.isfinite(args.isolation_max_load1)
            or args.isolation_max_load1 < 0
            or not math.isfinite(args.isolation_monitor_interval)
            or args.isolation_monitor_interval <= 0
            or (args.isolation_samples and not forbidden_names)):
        parser.error("invalid performance isolation configuration")
    if ((args.max_regression is not None or args.max_rclcpp_ratio is not None)
            and (args.isolation_samples < 3 or args.isolation_max_load1 > 2
                 or args.isolation_monitor_interval > 0.25)):
        parser.error("formal performance gates require strict continuous isolation")
    binaries = {"rust": args.rust.resolve(), "cpp": args.cpp.resolve()}
    if args.baseline_rust:
        binaries["baseline"] = args.baseline_rust.resolve()
    status_paths = {"rust": args.rust_status_path, "cpp": "per_goal"}
    if args.baseline_rust:
        status_paths["baseline"] = args.baseline_status_path
    feedback_delivery_paths = {
        "rust": args.rust_feedback_delivery,
        "cpp": "rclcpp_callback",
    }
    if args.baseline_rust:
        feedback_delivery_paths["baseline"] = args.baseline_feedback_delivery
    env = os.environ.copy()
    env.update(RMW_IMPLEMENTATION=args.rmw, ROS_DOMAIN_ID=str(args.domain_id), ROS_LOCALHOST_ONLY="1")
    env.pop("CYCLONEDDS_URI", None)
    env.pop("FASTRTPS_DEFAULT_PROFILES_FILE", None)
    env.pop("FASTDDS_DEFAULT_PROFILES_FILE", None)
    env["R2R_ACTION_FEEDBACK_STATUS_QOS"] = args.status_qos
    env["R2R_ACTION_FEEDBACK_RESULT_TIMEOUT_MS"] = str(args.result_timeout_ms)
    env["R2R_ACTION_FEEDBACK_STATUS_BATCH_SIZE"] = str(args.status_batch_size)
    env["R2R_ACTION_FEEDBACK_STATUS_SYNC"] = args.status_sync
    if args.probe:
        env["LD_PRELOAD"] = str(args.probe.resolve()) + (":" + env["LD_PRELOAD"] if env.get("LD_PRELOAD") else "")
    root = Path(__file__).resolve().parents[2]
    sources = [Path("r2r/benches/action_feedback_roundtrip.rs"),
               Path("tools/bench_ros/action_feedback.cpp"),
               Path("tools/bench_ros/action_feedback.py"),
               Path("tools/bench_ros/verify_action_feedback.py"),
               Path("tools/bench_ros/pubsub_pipeline.py"),
               Path("tools/bench_ros/callback_group_support/validation.py")]
    metadata = {
        "started_utc": datetime.datetime.now(datetime.timezone.utc).isoformat(),
        "workload": "action_feedback", "cpu_ping": args.cpu_ping, "cpu_echo": args.cpu_echo,
        "domain_id": args.domain_id, "rmw": args.rmw, "ros_distro": env["ROS_DISTRO"],
        "platform": platform.platform(),
        "cpu_model": next(line.split(":", 1)[1].strip() for line in Path("/proc/cpuinfo").read_text().splitlines() if line.startswith("model name")),
        "git_head": subprocess.check_output(["git", "rev-parse", "HEAD"], text=True).strip(),
        "working_tree_dirty": bool(subprocess.check_output(["git", "status", "--porcelain"], text=True).strip()),
        "rustc": subprocess.check_output(["rustc", "--version"], text=True).strip(),
        "cxx": subprocess.check_output(["c++", "--version"], text=True).splitlines()[0],
        "binary_sha256": {label: hashlib.sha256(path.read_bytes()).hexdigest() for label, path in binaries.items()},
        "source_sha256": {str(path): hashlib.sha256((root / path).read_bytes()).hexdigest() for path in sources},
        "payload_sizes": sizes, "goal_windows": windows,
        "feedback_per_goal": args.feedback_per_goal,
        "iterations": args.iterations, "warmup": args.warmup, "repetitions": args.repetitions,
        "qos": ("goal/result/cancel/feedback reliable volatile "
                "KeepLast(max(16, goal_window*feedback_per_goal)); "
                f"status {args.status_qos}; result retention {args.result_timeout_ms} ms"),
        "transport": "independent processes, localhost DDS, intra-process disabled",
        "scheduling": "single-threaded executor/spin loop; 100 ms wait cap; goal_window active goals",
        "protocol": ("submit the complete goal window; accept all goals; client observes every Executing status; readiness phase 0; server publishes round-robin feedback; client validates all payloads and sends phase 1 ACK; server succeeds every result; final rejected control goal" if args.status_sync=="required" else "submit the complete goal window; accept all goals; readiness phase 0; server publishes round-robin feedback; client validates all payloads and sends phase 1 ACK; server succeeds every result; final rejected control goal; status and feedback competition is retained"),
        "rtt_boundary": "before first goal submission through all accepted goals, readiness handshake, all validated feedback, acknowledgement and all successful results",
        "feedback_boundary": ("after every accepted goal is observed Executing, before readiness phase 0 request, through complete validated round-robin feedback consumption" if args.status_sync=="required" else "after accepted-goal retrieval, before readiness phase 0 request, through complete validated round-robin feedback consumption; status and feedback competition is included"),
        "throughput_boundary": "all measured batches including full payload validation; excludes discovery, warmup and final control",
        "instrumentation": {"enabled": bool(args.probe), "latency_evidence": not bool(args.probe)},
        "matrix_scope": args.matrix_scope,
        "status_qos": args.status_qos,
        "result_timeout_ms": args.result_timeout_ms,
        "status_batch_size": args.status_batch_size,
        "status_paths": status_paths,
        "feedback_delivery_paths": feedback_delivery_paths,
        "status_sync": args.status_sync,
        "server_stage_labels": list(SERVER_STAGES),
        "server_stage_boundaries": {
            "accept": "wait for and accept the complete goal window",
            "start_wait": "wait for the phase-0 readiness request; includes response send in rclcpp",
            "execute": "transition every accepted goal to executing",
            "feedback_publish": "construct and publish every round-robin feedback payload",
            "feedback_wait": "wait for the phase-1 acknowledgement after complete client validation",
            "terminal": "succeed every goal and publish terminal results/status"
        },
    }
    if args.probe:
        metadata["instrumentation"].update(
            probe_sha256=hashlib.sha256(args.probe.read_bytes()).hexdigest(),
            scope="process-wide allocation entry points and getrusage during measured windows; Rust future wakes only")
    if args.build_manifest:
        metadata["build_manifest"] = json.loads(args.build_manifest.read_text())
    prefix = Path("/opt/ros") / env["ROS_DISTRO"] / "share"
    packages = ["rclcpp", "rclcpp_action", "rcl_action", "rcl", "rmw", "example_interfaces", args.rmw]
    metadata["ros_packages"] = {name: ET.parse(prefix / name / "package.xml").findtext("version") for name in packages}
    pairs = matrix_pairs(args.baseline_rust is not None, args.matrix_scope)
    monitor = None
    if args.isolation_samples:
        try:
            preflight = wait_for_isolation(
                args.isolation_samples, args.isolation_interval, args.isolation_timeout,
                args.isolation_max_load1, forbidden_names)
        except RuntimeError as error:
            metadata["isolation_gate"] = {
                "passed": False,
                "preflight_error": str(error),
                "measurement_started": False,
            }
            args.output.parent.mkdir(parents=True, exist_ok=True)
            args.output.write_text(json.dumps({"metadata": metadata, "records": [],
                                               "summary": []}, indent=2) + "\n")
            raise
        monitor = IsolationMonitor(
            forbidden_names, args.isolation_monitor_interval, args.isolation_max_load1)
        monitor.start()
    records, traces = [], []
    for elements in sizes:
        for goal_window in windows:
            for repetition in range(args.repetitions):
                ordered_pairs = pairs if repetition % 2 == 0 else reversed(pairs)
                for ping_label, echo_label in ordered_pairs:
                    run_prefix = "/r2r_action_feedback_" + uuid.uuid4().hex
                    ping, echo = pair(
                        binaries[ping_label], binaries[echo_label], args.cpu_ping, args.cpu_echo,
                        env, elements, goal_window, args.feedback_per_goal, args.iterations,
                        args.warmup, run_prefix, bool(args.probe), True, args.status_qos,
                        status_paths[ping_label], status_paths[echo_label], args.status_sync,
                        feedback_delivery_paths[ping_label], feedback_delivery_paths[echo_label])
                    validate_endpoint(ping, "ping", args.rmw, elements, goal_window,
                                      args.feedback_per_goal, args.iterations, args.warmup,
                                      "rclcpp" if ping_label == "cpp" else "r2r", bool(args.probe), True,
                                      args.status_qos, args.result_timeout_ms,
                                      args.status_batch_size, status_paths[ping_label], args.status_sync,
                                      feedback_delivery_paths[ping_label])
                    validate_endpoint(echo, "echo", args.rmw, elements, goal_window,
                                      args.feedback_per_goal, args.iterations, args.warmup,
                                      "rclcpp" if echo_label == "cpp" else "r2r", bool(args.probe), True,
                                      args.status_qos, args.result_timeout_ms,
                                      args.status_batch_size, status_paths[echo_label], args.status_sync)
                    samples = ping.pop("samples_ns")
                    feedback_samples = ping.pop("feedback_samples_ns")
                    server_stage_samples = echo.pop("server_stage_samples_ns")
                    traces.append({"record": len(records), "samples_ns": samples,
                                   "feedback_samples_ns": feedback_samples,
                                   "server_stage_samples_ns": server_stage_samples})
                    records.append({"elements": elements, "goal_window": goal_window,
                                    "ping_language": ping_label, "echo_language": echo_label,
                                    "repetition": repetition, "ping": ping, "echo": echo})
                    print(f"{ping_label}/{echo_label} elements={elements} goals={goal_window} "
                          f"repetition={repetition + 1}/{args.repetitions}: "
                          f"feedback p50={ping['feedback_ns_p50']} ns p99={ping['feedback_ns_p99']} ns", flush=True)
    if monitor is not None:
        runtime_isolation = monitor.finish()
        metadata["isolation_gate"] = {
            "passed": preflight["passed"] and runtime_isolation["passed"],
            "preflight": preflight,
            "monitor": runtime_isolation,
        }
    fields = ["rtt_ns_p50", "rtt_ns_p95", "rtt_ns_p99", "feedback_ns_p50",
              "feedback_ns_p95", "feedback_ns_p99", "roundtrips_per_second",
              "feedback_messages_per_second"]
    summary = []
    for elements in sizes:
        for goal_window in windows:
            for ping_label, echo_label in pairs:
                group = [record["ping"] for record in records
                         if (record["elements"], record["goal_window"], record["ping_language"], record["echo_language"])
                         == (elements, goal_window, ping_label, echo_label)]
                echo_group = [record["echo"] for record in records
                              if (record["elements"], record["goal_window"], record["ping_language"], record["echo_language"])
                              == (elements, goal_window, ping_label, echo_label)]
                row = {"elements": elements, "goal_window": goal_window,
                       "ping_language": ping_label, "echo_language": echo_label,
                       **{field: statistics.median(record[field] for record in group) for field in fields}}
                for percent in [50, 95, 99]:
                    row[f"server_stage_ns_p{percent}"] = {
                        stage: statistics.median(record[f"server_stage_ns_p{percent}"][stage]
                                                 for record in echo_group)
                        for stage in SERVER_STAGES
                    }
                summary.append(row)
    args.output.parent.mkdir(parents=True, exist_ok=True)
    trace_path = args.output.with_suffix(".samples.json.gz")
    compressed = gzip.compress(json.dumps(traces, separators=(",", ":")).encode(), mtime=0)
    trace_path.write_bytes(compressed)
    metadata["samples_artifact"] = {"file": trace_path.name,
                                    "sha256": hashlib.sha256(compressed).hexdigest(),
                                    "record_count": len(traces)}
    metadata["completed_utc"] = datetime.datetime.now(datetime.timezone.utc).isoformat()
    failures = regression_failures(summary, sizes, windows, args.max_regression) if args.max_regression is not None else []
    if args.max_regression is not None:
        metadata["regression_gate"] = {"maximum_ratio": args.max_regression,
                                       "passed": not failures, "failures": failures}
    cpp_failures = (rclcpp_failures(summary, sizes, windows, args.max_rclcpp_ratio)
                    if args.max_rclcpp_ratio is not None else [])
    if args.max_rclcpp_ratio is not None:
        metadata["rclcpp_gate"] = {
            "maximum_ratio": args.max_rclcpp_ratio,
            "fields": ["rtt_ns_p50", "rtt_ns_p95", "rtt_ns_p99",
                       "feedback_ns_p50", "feedback_ns_p95", "feedback_ns_p99",
                       "inverse_throughput"],
            "passed": not cpp_failures,
            "failures": cpp_failures,
        }
    args.output.write_text(json.dumps({"metadata": metadata, "records": records, "summary": summary}, indent=2) + "\n")
    print(json.dumps(summary, indent=2))
    isolation_failed = ("isolation_gate" in metadata
                        and not metadata["isolation_gate"]["passed"])
    if failures or cpp_failures or isolation_failed:
        raise RuntimeError(
            f"action feedback gate failed: regression={failures}, rclcpp={cpp_failures}, "
            f"isolation={isolation_failed}")


if __name__ == "__main__":
    main()
