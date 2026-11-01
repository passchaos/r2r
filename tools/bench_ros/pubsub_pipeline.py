#!/usr/bin/env python3
"""Measure pipelined ROS pub/sub RTT and echo-side subscription fanout."""
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
import threading
import time
import uuid
import xml.etree.ElementTree as ET


PING_FIELDS = (
    "rtt_ns_p50", "rtt_ns_p95", "rtt_ns_p99",
    "publish_ns_p50", "publish_ns_p95", "publish_ns_p99",
)
ECHO_FIELDS = (
    "collect_ns_p50", "collect_ns_p95", "collect_ns_p99",
    "publish_ns_p50", "publish_ns_p95", "publish_ns_p99",
)
ALL_LATENCY_FIELDS = PING_FIELDS + tuple(f"echo_{field}" for field in ECHO_FIELDS)
GATE_FIELDS = ("rtt_ns_p50", "rtt_ns_p95", "rtt_ns_p99")
DEFAULT_FORBIDDEN_PROCESS_NAMES = (
    "zig", "rustc", "cargo", "cc1", "cc1plus", "clang", "clang++", "gcc", "g++",
    "collect2", "ld", "ld.lld", "lld", "mold", "cmake", "make", "ninja",
)


def competing_process_counts(names):
    names = set(names)
    counts = {}
    for entry in Path("/proc").iterdir():
        if not entry.name.isdigit():
            continue
        try:
            name = (entry / "comm").read_text().strip()
        except (FileNotFoundError, PermissionError, ProcessLookupError):
            continue
        if name in names:
            counts[name] = counts.get(name, 0) + 1
    return counts


def isolation_snapshot(forbidden_names):
    return {
        "observed_utc": datetime.datetime.now(datetime.timezone.utc).isoformat(),
        "load1": os.getloadavg()[0],
        "competing_processes": competing_process_counts(forbidden_names),
    }


def snapshot_is_idle(snapshot, maximum_load1):
    return snapshot["load1"] <= maximum_load1 and not snapshot["competing_processes"]


def wait_for_isolation(sample_count, interval, timeout, maximum_load1, forbidden_names,
                       snapshot=isolation_snapshot, sleeper=time.sleep,
                       monotonic=time.monotonic):
    deadline = monotonic() + timeout
    consecutive = []
    while True:
        observed = snapshot(forbidden_names)
        if snapshot_is_idle(observed, maximum_load1):
            consecutive.append(observed)
            if len(consecutive) == sample_count:
                return {
                    "required_consecutive_samples": sample_count,
                    "interval_seconds": interval,
                    "timeout_seconds": timeout,
                    "maximum_load1": maximum_load1,
                    "forbidden_process_names": list(forbidden_names),
                    "samples": consecutive,
                    "passed": True,
                }
        else:
            consecutive.clear()
        if monotonic() + interval > deadline:
            raise RuntimeError(f"performance isolation preflight timed out: {observed}")
        sleeper(interval)


class IsolationMonitor:
    def __init__(self, forbidden_names, interval, maximum_load1):
        self.forbidden_names = tuple(forbidden_names)
        self.interval = interval
        self.maximum_load1 = maximum_load1
        self.stop_event = threading.Event()
        self.lock = threading.Lock()
        self.observation_count = 0
        self.observed_maximum_load1 = 0.0
        self.violations = []
        self.thread = threading.Thread(target=self._run, name="pipeline-isolation", daemon=True)

    def _observe(self):
        observed = isolation_snapshot(self.forbidden_names)
        with self.lock:
            self.observation_count += 1
            self.observed_maximum_load1 = max(self.observed_maximum_load1, observed["load1"])
            if not snapshot_is_idle(observed, self.maximum_load1) and len(self.violations) < 64:
                self.violations.append(observed)

    def _run(self):
        self._observe()
        while not self.stop_event.wait(self.interval):
            self._observe()

    def start(self):
        self.thread.start()

    def finish(self):
        self.stop_event.set()
        self.thread.join()
        self._observe()
        with self.lock:
            return {
                "interval_seconds": self.interval,
                "maximum_allowed_load1": self.maximum_load1,
                "observation_count": self.observation_count,
                "maximum_load1": self.observed_maximum_load1,
                "violations": list(self.violations),
                "passed": not self.violations,
            }


def percentile(samples, percent):
    ordered = sorted(samples)
    return ordered[math.ceil((len(ordered) - 1) * percent / 100)]


def parse_cases(value):
    cases = []
    for item in value.split(","):
        fields = item.split(":")
        if len(fields) != 3:
            raise ValueError("pipeline cases must use elements:window:fanout")
        case = tuple(int(field) for field in fields)
        if case in cases:
            raise ValueError("duplicate pipeline case")
        cases.append(case)
    if (not cases or any(not 0 <= elements <= 1_000_000 or not 1 <= window <= 4096
                         or not 1 <= fanout <= 64 or window * fanout > 4096
                         for elements, window, fanout in cases)):
        raise ValueError("invalid pipeline case")
    return cases


def matrix_pairs(baseline, scope):
    if scope == "full" and not baseline:
        return [("rust", "rust"), ("cpp", "cpp"), ("rust", "cpp"), ("cpp", "rust")]
    if scope == "full" and baseline:
        return [("baseline", "baseline"), ("rust", "rust"), ("cpp", "cpp"),
                ("rust", "cpp"), ("cpp", "rust"), ("baseline", "cpp"),
                ("cpp", "baseline")]
    if scope == "regression" and baseline:
        return [("baseline", "baseline"), ("rust", "rust")]
    raise RuntimeError("regression scope requires an independent baseline")


def reserve_measurement_cpus(allowed, cpu_ping, cpu_echo, set_affinity=os.sched_setaffinity):
    measurement = {cpu_ping, cpu_echo}
    if cpu_ping == cpu_echo or not measurement.issubset(allowed):
        raise ValueError("select two distinct available CPUs")
    orchestrator = set(allowed) - measurement
    if not orchestrator:
        raise ValueError("at least one non-measurement CPU is required for the runner")
    set_affinity(0, orchestrator)
    return sorted(orchestrator)


def validate_endpoint(result, role, rmw, elements, window, fanout, iterations, warmup,
                      implementation=None, receive_path=None, raw=True):
    if implementation is not None and result.get("implementation") != implementation:
        raise RuntimeError("pipeline implementation mismatch")
    identity = (result.get("role"), result.get("rmw"), result.get("elements"),
                result.get("window"), result.get("fanout"), result.get("iterations"),
                result.get("warmup"))
    expected = (role, rmw, elements, window, fanout, iterations, warmup)
    if identity != expected:
        raise RuntimeError("pipeline endpoint identity mismatch")
    if receive_path is not None and result.get("receive_path") != receive_path:
        raise RuntimeError("pipeline receive path mismatch")
    rss = result.get("process_max_rss_kib")
    if rss is not None and (type(rss) is not int or rss <= 0):
        raise RuntimeError("invalid pipeline process RSS")
    total = iterations + warmup
    checksum = total * (total - 1) // 2
    expected_inflight = min(window, max(iterations, warmup))
    if (result.get("received") != total or result.get("fanout_received") != total * fanout
            or result.get("checksum") != checksum
            or result.get("fanout_checksum") != checksum * fanout
            or result.get("max_inflight") != expected_inflight):
        raise RuntimeError("pipeline count, checksum or window oracle mismatch")
    raw_fields = ("samples_ns", "publish_samples_ns") if role == "ping" else (
        "collect_samples_ns", "publish_samples_ns")
    if not raw:
        if any(field in result for field in raw_fields):
            raise RuntimeError("undeclared inline pipeline samples")
        return
    for field in raw_fields:
        samples = result.get(field)
        if (not isinstance(samples, list) or len(samples) != iterations
                or any(type(sample) is not int or sample <= 0 for sample in samples)):
            raise RuntimeError("invalid pipeline stage samples")
    if role == "ping":
        samples = result["samples_ns"]
        publish_samples = result["publish_samples_ns"]
        if any(publish > rtt for publish, rtt in zip(publish_samples, samples)):
            raise RuntimeError("pipeline publish stage exceeds RTT")
        for percent in [50, 95, 99]:
            if result.get(f"rtt_ns_p{percent}") != percentile(samples, percent):
                raise RuntimeError("pipeline RTT percentile mismatch")
            if result.get(f"publish_ns_p{percent}") != percentile(publish_samples, percent):
                raise RuntimeError("pipeline publish percentile mismatch")
        if result.get("rtt_ns_max") != max(samples):
            raise RuntimeError("pipeline RTT maximum mismatch")
        elapsed = result.get("elapsed_ns")
        rate = result.get("messages_per_second")
        if (type(elapsed) is not int or elapsed < max(samples) or not isinstance(rate, (int, float))
                or not math.isfinite(rate) or rate <= 0
                or not math.isclose(rate, iterations / (elapsed * 1e-9), rel_tol=1e-5)):
            raise RuntimeError("pipeline elapsed time or throughput mismatch")
    else:
        for prefix, samples in [("collect", result["collect_samples_ns"]),
                                ("publish", result["publish_samples_ns"])]:
            for percent in [50, 95, 99]:
                if result.get(f"{prefix}_ns_p{percent}") != percentile(samples, percent):
                    raise RuntimeError(f"pipeline echo {prefix} percentile mismatch")


def regression_failures(summary, cases, maximum, fields=GATE_FIELDS):
    failures = []
    for elements, window, fanout in cases:
        identity = (elements, window, fanout)
        baseline = next(row for row in summary if (row["elements"], row["window"], row["fanout"]) == identity
                        and row["ping_language"] == row["echo_language"] == "baseline")
        candidate = next(row for row in summary if (row["elements"], row["window"], row["fanout"]) == identity
                         and row["ping_language"] == row["echo_language"] == "rust")
        ratios = {field: candidate[field] / baseline[field] for field in fields}
        ratios["inverse_throughput"] = baseline["messages_per_second"] / candidate["messages_per_second"]
        if any(ratio > maximum for ratio in ratios.values()):
            failures.append({"elements": elements, "window": window, "fanout": fanout,
                             "ratios": ratios})
    return failures


def rclcpp_failures(summary, cases, maximum, fields=GATE_FIELDS):
    failures = []
    for elements, window, fanout in cases:
        identity = (elements, window, fanout)
        rust = next(row for row in summary
                    if (row["elements"], row["window"], row["fanout"]) == identity
                    and row["ping_language"] == row["echo_language"] == "rust")
        cpp = next(row for row in summary
                   if (row["elements"], row["window"], row["fanout"]) == identity
                   and row["ping_language"] == row["echo_language"] == "cpp")
        ratios = {field: rust[field] / cpp[field] for field in fields}
        ratios["inverse_throughput"] = cpp["messages_per_second"] / rust["messages_per_second"]
        if any(ratio > maximum for ratio in ratios.values()):
            failures.append({"elements": elements, "window": window, "fanout": fanout,
                             "ratios": ratios})
    return failures


def sample_peak_rss(pid, stop, peak):
    path = Path(f"/proc/{pid}/status")
    while not stop.is_set():
        try:
            text = path.read_text()
        except (FileNotFoundError, ProcessLookupError):
            break
        for line in text.splitlines():
            if line.startswith("VmHWM:"):
                peak[0] = max(peak[0], int(line.split()[1]))
                break
        stop.wait(0.005)


def pair(ping, echo, cpu_ping, cpu_echo, env, case, iterations, warmup, prefix,
         ping_receive_path, echo_receive_path):
    parameters = [*(str(value) for value in case), str(iterations), str(warmup), prefix]
    with tempfile.TemporaryDirectory(prefix="r2r-pubsub-pipeline-") as directory:
        output = Path(directory) / "echo.json"
        errors = Path(directory) / "echo.log"
        with output.open("w") as out, errors.open("w") as err:
            responder = subprocess.Popen(
                ["taskset", "-c", str(cpu_echo), str(echo), "echo", *parameters],
                env=env, stdout=out, stderr=err)
            initiator = subprocess.Popen(
                ["taskset", "-c", str(cpu_ping), str(ping), "ping", *parameters],
                env=env, text=True, stdout=subprocess.PIPE, stderr=subprocess.PIPE)
            stop = threading.Event()
            ping_peak, echo_peak = [0], [0]
            ping_thread = threading.Thread(
                target=sample_peak_rss, args=(initiator.pid, stop, ping_peak), daemon=True)
            echo_thread = threading.Thread(
                target=sample_peak_rss, args=(responder.pid, stop, echo_peak), daemon=True)
            ping_thread.start()
            echo_thread.start()
            try:
                initiator_stdout, initiator_stderr = initiator.communicate(timeout=180)
                if initiator.returncode:
                    raise RuntimeError(
                        f"pipeline ping failed ({initiator.returncode}): {initiator_stderr}\n"
                        f"echo: {errors.read_text()}")
                if responder.wait(timeout=30):
                    raise RuntimeError(f"pipeline echo failed: {errors.read_text()}")
                ping_result = json.loads(initiator_stdout)
                echo_result = json.loads(output.read_text())
            finally:
                stop.set()
                ping_thread.join(timeout=1)
                echo_thread.join(timeout=1)
                if responder.poll() is None:
                    responder.terminate()
                    try:
                        responder.wait(timeout=5)
                    except subprocess.TimeoutExpired:
                        responder.kill()
                        responder.wait()
            ping_result["process_max_rss_kib"] = ping_peak[0]
            echo_result["process_max_rss_kib"] = echo_peak[0]
    for result, role, receive_path in [
            (ping_result, "ping", ping_receive_path),
            (echo_result, "echo", echo_receive_path)]:
        validate_endpoint(result, role, env["RMW_IMPLEMENTATION"], *case, iterations, warmup,
                          receive_path=receive_path)
    return ping_result, echo_result


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--rust", type=Path, required=True)
    parser.add_argument("--baseline-rust", type=Path)
    parser.add_argument("--cpp", type=Path, required=True)
    parser.add_argument("--cpu-ping", type=int, required=True)
    parser.add_argument("--cpu-echo", type=int, required=True)
    parser.add_argument("--domain-id", type=int, default=120)
    parser.add_argument("--rmw", default="rmw_cyclonedds_cpp")
    parser.add_argument("--cases", default="0:1:1,4096:16:1,4096:16:4")
    parser.add_argument("--iterations", type=int, default=1000)
    parser.add_argument("--warmup", type=int, default=50)
    parser.add_argument("--repetitions", type=int, default=5)
    parser.add_argument("--matrix-scope", choices=["full", "regression"], default="full")
    parser.add_argument("--rust-receive-path", choices=["stream", "native_callback", "reusable_typed", "batch_typed"],
                        default="native_callback")
    parser.add_argument("--baseline-receive-path", choices=["stream", "native_callback", "reusable_typed", "batch_typed"],
                        default="stream")
    parser.add_argument("--max-regression", type=float)
    parser.add_argument("--max-rclcpp-ratio", type=float,
                        help="maximum Rust/Rust to C++/C++ ratio for RTT and inverse throughput")
    parser.add_argument("--build-manifest", type=Path)
    parser.add_argument("--isolation-samples", type=int, default=0,
                        help="required consecutive idle samples before measurement (zero disables)")
    parser.add_argument("--isolation-interval", type=float, default=20.0)
    parser.add_argument("--isolation-timeout", type=float, default=300.0)
    parser.add_argument("--isolation-max-load1", type=float, default=2.0)
    parser.add_argument("--isolation-monitor-interval", type=float, default=0.25)
    parser.add_argument("--isolation-forbid-comm", default=",".join(DEFAULT_FORBIDDEN_PROCESS_NAMES),
                        help="comma-separated /proc comm names forbidden during formal runs")
    parser.add_argument("--output", type=Path, required=True)
    args = parser.parse_args()
    try:
        cases = parse_cases(args.cases)
    except ValueError as error:
        parser.error(str(error))
    allowed = os.sched_getaffinity(0)
    try:
        orchestrator_cpus = reserve_measurement_cpus(allowed, args.cpu_ping, args.cpu_echo)
    except ValueError as error:
        parser.error(str(error))
    if (not 0 <= args.domain_id <= 232 or args.iterations <= 0 or args.warmup < 0
            or args.repetitions < 1 or args.iterations + args.warmup > 2**32 - 1):
        parser.error("invalid pipeline run dimensions")
    if args.matrix_scope == "regression" and args.baseline_rust is None:
        parser.error("regression scope requires an independent baseline")
    if args.max_regression is not None and (args.baseline_rust is None
            or not math.isfinite(args.max_regression) or args.max_regression < 1):
        parser.error("a regression limit needs a baseline and a finite ratio at least one")
    if args.max_rclcpp_ratio is not None and (
            args.matrix_scope != "full" or not math.isfinite(args.max_rclcpp_ratio)
            or args.max_rclcpp_ratio <= 0):
        parser.error("an rclcpp limit needs full scope and a finite positive ratio")
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
            and args.isolation_samples < 3):
        parser.error("formal performance gates require at least three idle isolation samples")
    binaries = {"rust": args.rust.resolve(), "cpp": args.cpp.resolve()}
    if args.baseline_rust:
        binaries["baseline"] = args.baseline_rust.resolve()
    pairs = matrix_pairs(args.baseline_rust is not None, args.matrix_scope)
    receive_paths = {"rust": args.rust_receive_path, "cpp": "callback"}
    if args.baseline_rust:
        receive_paths["baseline"] = args.baseline_receive_path
    env = os.environ.copy()
    env.update(RMW_IMPLEMENTATION=args.rmw, ROS_DOMAIN_ID=str(args.domain_id), ROS_LOCALHOST_ONLY="1")
    env.pop("CYCLONEDDS_URI", None)
    env.pop("FASTRTPS_DEFAULT_PROFILES_FILE", None)
    env.pop("FASTDDS_DEFAULT_PROFILES_FILE", None)
    root = Path(__file__).resolve().parents[2]
    sources = [Path("r2r/benches/pubsub_pipeline.rs"),
               Path("tools/bench_ros/pubsub_pipeline.cpp"),
               Path("tools/bench_ros/pubsub_pipeline.py"),
               Path("tools/bench_ros/build.py"),
               Path("tools/bench_ros/verify_pubsub_pipeline.py"),
               Path("tools/bench_ros/test_pubsub_pipeline.py")]
    metadata = {
        "started_utc": datetime.datetime.now(datetime.timezone.utc).isoformat(),
        "workload": "pubsub_pipeline", "cpu_ping": args.cpu_ping, "cpu_echo": args.cpu_echo,
        "orchestrator_cpu_affinity": orchestrator_cpus,
        "domain_id": args.domain_id, "rmw": args.rmw, "ros_distro": env["ROS_DISTRO"],
        "platform": platform.platform(),
        "cpu_model": next(line.split(":", 1)[1].strip() for line in Path("/proc/cpuinfo").read_text().splitlines() if line.startswith("model name")),
        "git_head": subprocess.check_output(["git", "rev-parse", "HEAD"], text=True).strip(),
        "working_tree_dirty": bool(subprocess.check_output(["git", "status", "--porcelain"], text=True).strip()),
        "rustc": subprocess.check_output(["rustc", "--version"], text=True).strip(),
        "cxx": subprocess.check_output(["c++", "--version"], text=True).splitlines()[0],
        "binary_sha256": {label: hashlib.sha256(path.read_bytes()).hexdigest()
                           for label, path in binaries.items()},
        "source_sha256": {str(path): hashlib.sha256((root / path).read_bytes()).hexdigest()
                          for path in sources},
        "cases": [{"elements": elements, "window": window, "fanout": fanout}
                  for elements, window, fanout in cases],
        "iterations": args.iterations, "warmup": args.warmup, "repetitions": args.repetitions,
        "matrix_scope": args.matrix_scope,
        "receive_paths": receive_paths,
        "regression_fields": [*GATE_FIELDS, "inverse_throughput"],
        "qos": "reliable, volatile, KeepLast(max(16, window))",
        "transport": "independent processes, localhost DDS, intra-process disabled",
        "scheduling": "single-threaded executor/spin loop; fixed request window; echo-side subscription fanout",
        "protocol": "separate warmup and measured pipelines; each request is validated by every echo subscription; one ordered response per sequence; final empty control reaches every fanout subscription",
        "rtt_boundary": "before request publish through ordered response retrieval and initiator validation",
        "throughput_boundary": "complete measured pipeline including payload validation; excludes discovery, warmup and teardown",
        "echo_stage_boundaries": {"collect": "wait for and validate one copy from every echo subscription",
                                  "publish": "normal typed response publication"},
    }
    if args.build_manifest:
        metadata["build_manifest"] = json.loads(args.build_manifest.read_text())
    prefix = Path("/opt/ros") / env["ROS_DISTRO"] / "share"
    metadata["ros_packages"] = {name: ET.parse(prefix / name / "package.xml").findtext("version")
                                for name in ["rclcpp", "rcl", "rmw", "std_msgs", args.rmw]}
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
    try:
        for case in cases:
            for repetition in range(args.repetitions):
                ordered_pairs = pairs if repetition % 2 == 0 else reversed(pairs)
                for ping_label, echo_label in ordered_pairs:
                    run_prefix = "/r2r_pubsub_pipeline_" + uuid.uuid4().hex
                    ping, echo = pair(binaries[ping_label], binaries[echo_label], args.cpu_ping,
                                      args.cpu_echo, env, case, args.iterations, args.warmup,
                                      run_prefix, receive_paths[ping_label],
                                      receive_paths[echo_label])
                    validate_endpoint(ping, "ping", args.rmw, *case, args.iterations, args.warmup,
                                      "rclcpp" if ping_label == "cpp" else "r2r",
                                      receive_paths[ping_label])
                    validate_endpoint(echo, "echo", args.rmw, *case, args.iterations, args.warmup,
                                      "rclcpp" if echo_label == "cpp" else "r2r",
                                      receive_paths[echo_label])
                    ping_samples = ping.pop("samples_ns")
                    ping_publish = ping.pop("publish_samples_ns")
                    echo_collect = echo.pop("collect_samples_ns")
                    echo_publish = echo.pop("publish_samples_ns")
                    traces.append({"record": len(records), "samples_ns": ping_samples,
                                   "ping_publish_samples_ns": ping_publish,
                                   "echo_collect_samples_ns": echo_collect,
                                   "echo_publish_samples_ns": echo_publish})
                    records.append({"elements": case[0], "window": case[1], "fanout": case[2],
                                    "ping_language": ping_label, "echo_language": echo_label,
                                    "repetition": repetition, "ping": ping, "echo": echo})
                    print(f"{ping_label}/{echo_label} elements={case[0]} window={case[1]} "
                          f"fanout={case[2]} repetition={repetition + 1}/{args.repetitions}: "
                          f"p50={ping['rtt_ns_p50']} ns p99={ping['rtt_ns_p99']} ns", flush=True)
    finally:
        if monitor is not None:
            runtime_isolation = monitor.finish()
            metadata["isolation_gate"] = {
                "passed": preflight["passed"] and runtime_isolation["passed"],
                "preflight": preflight,
                "runtime": runtime_isolation,
            }
    summary = []
    for case in cases:
        for ping_label, echo_label in pairs:
            group = [record for record in records
                     if (record["elements"], record["window"], record["fanout"]) == case
                     and record["ping_language"] == ping_label and record["echo_language"] == echo_label]
            row = {"elements": case[0], "window": case[1], "fanout": case[2],
                   "ping_language": ping_label, "echo_language": echo_label}
            for field in PING_FIELDS + ("messages_per_second",):
                row[field] = statistics.median(record["ping"][field] for record in group)
            for field in ECHO_FIELDS:
                row[f"echo_{field}"] = statistics.median(record["echo"][field] for record in group)
            summary.append(row)
    args.output.parent.mkdir(parents=True, exist_ok=True)
    trace_path = args.output.with_suffix(".samples.json.gz")
    compressed = gzip.compress(json.dumps(traces, separators=(",", ":")).encode(), mtime=0)
    trace_path.write_bytes(compressed)
    metadata["samples_artifact"] = {"file": trace_path.name,
                                    "sha256": hashlib.sha256(compressed).hexdigest(),
                                    "record_count": len(traces)}
    metadata["completed_utc"] = datetime.datetime.now(datetime.timezone.utc).isoformat()
    failures = regression_failures(summary, cases, args.max_regression) if args.max_regression is not None else []
    if args.max_regression is not None:
        metadata["regression_gate"] = {"maximum_ratio": args.max_regression,
                                       "passed": not failures, "failures": failures}
    cpp_failures = (rclcpp_failures(summary, cases, args.max_rclcpp_ratio)
                    if args.max_rclcpp_ratio is not None else [])
    if args.max_rclcpp_ratio is not None:
        metadata["rclcpp_gate"] = {
            "maximum_ratio": args.max_rclcpp_ratio,
            "fields": [*GATE_FIELDS, "inverse_throughput"],
            "passed": not cpp_failures,
            "failures": cpp_failures,
        }
    args.output.write_text(json.dumps({"metadata": metadata, "records": records,
                                      "summary": summary}, indent=2) + "\n")
    print(json.dumps(summary, indent=2))
    isolation_failed = ("isolation_gate" in metadata
                        and not metadata["isolation_gate"]["passed"])
    if failures or cpp_failures or isolation_failed:
        raise RuntimeError(
            f"pipeline gate failed: regression={failures}, rclcpp={cpp_failures}, "
            f"isolation={isolation_failed}")


if __name__ == "__main__":
    main()
