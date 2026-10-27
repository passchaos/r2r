"""Independent action-cohort and probe validation; no live process access."""

ALLOCATION_FIELDS = {"malloc_calls", "calloc_calls", "realloc_calls", "free_calls", "null_free_calls",
                     "aligned_calls", "failed_allocations", "requested_bytes", "size_overflows", "counter_overflows"}
RESOURCE_FIELDS = {"user_cpu_ns", "system_cpu_ns", "voluntary_context_switches", "involuntary_context_switches", "process_max_rss_kib"}


def language_pairs(baseline, scope="full"):
    if scope == "action_stability":
        if not baseline:
            raise RuntimeError("action stability requires independent baseline/candidate binaries")
        return [("baseline", "baseline"), ("rust", "rust")]
    if scope != "full":
        raise RuntimeError("unknown benchmark matrix scope")
    pairs = [("rust", "rust"), ("cpp", "cpp"), ("rust", "cpp"), ("cpp", "rust")]
    return [("baseline", "baseline"), *pairs, ("baseline", "cpp"), ("cpp", "baseline")] if baseline else pairs


def validate_probe(frame, implementation):
    if not isinstance(frame, dict) or frame.get("abi") != 1:
        raise RuntimeError("invalid probe ABI")
    for name, fields in [("allocations", ALLOCATION_FIELDS), ("resources", RESOURCE_FIELDS)]:
        group = frame.get(name)
        if not isinstance(group, dict) or set(group) != fields or any(type(value) is not int or not 0 <= value < 2**64 for value in group.values()):
            raise RuntimeError("invalid probe counters")
    allocation = frame["allocations"]
    if allocation["size_overflows"] or allocation["counter_overflows"]:
        raise RuntimeError("probe counter overflow")
    if allocation["failed_allocations"] > sum(allocation[name] for name in ["malloc_calls", "calloc_calls", "realloc_calls", "aligned_calls"]):
        raise RuntimeError("probe allocation failures exceed attempts")
    application = frame.get("application")
    if not isinstance(application, dict) or set(application) != {"future_polls", "spin_calls", "future_wakes"}:
        raise RuntimeError("invalid application probe counters")
    if any(type(application[name]) is not int or application[name] < 0 for name in ["future_polls", "spin_calls"]):
        raise RuntimeError("invalid application probe counters")
    wakes = application["future_wakes"]
    if implementation == "rclcpp" and wakes is not None:
        raise RuntimeError("C++ future wake count is unavailable")
    if implementation == "r2r" and (type(wakes) is not int or wakes < 0):
        raise RuntimeError("invalid Rust future wake count")


def validate_cohorts(result, role, scenario, iterations, warmup, expected=None, instrumented=None):
    config = result.get("cohort_config")
    if config is None:
        if expected not in {None, 1} or "cohorts" in result:
            raise RuntimeError("missing action cohort configuration")
        frames = [result.get("probe")]
    else:
        if not isinstance(config, dict) or set(config) != {"count", "iterations", "warmup", "maximum_retained_goals", "fresh_endpoints"}:
            raise RuntimeError("invalid action cohort configuration")
        count, measured, heated = (config[name] for name in ["count", "iterations", "warmup"])
        if any(type(value) is not int for value in [count, measured, heated]) or not 2 <= count <= 1024 or measured <= 0 or heated < 0:
            raise RuntimeError("invalid action cohort dimensions")
        if expected is not None and count != expected:
            raise RuntimeError("action cohort count mismatch")
        if measured * count != iterations or heated * count != warmup or type(config["maximum_retained_goals"]) is not int or config["maximum_retained_goals"] != measured + heated or config["fresh_endpoints"] is not True:
            raise RuntimeError("action cohort boundary mismatch")
        cohorts = result.get("cohorts")
        if not isinstance(cohorts, list) or len(cohorts) != count:
            raise RuntimeError("missing action cohorts")
        total = measured + heated
        elapsed = 0
        for index, cohort in enumerate(cohorts):
            if not isinstance(cohort, dict) or any(type(cohort.get(name)) is not int for name in ["index", "sequence_offset", "iterations", "warmup", "received", "accepted", "feedback", "results", "canceled", "checksum"]):
                raise RuntimeError("invalid action cohort integer fields")
            offset = index * total
            identity = (cohort.get("index"), cohort.get("sequence_offset"), cohort.get("iterations"), cohort.get("warmup"))
            if identity != (index, offset, measured, heated):
                raise RuntimeError("action cohort sequence mismatch")
            if any(cohort.get(name) != total for name in ["received", "accepted", "feedback", "results"]):
                raise RuntimeError("action cohort lifecycle mismatch")
            if cohort.get("canceled") != (total if scenario else 0) or cohort.get("checksum") != total * offset + total * (total - 1) // 2:
                raise RuntimeError("action cohort payload/count mismatch")
            if role == "ping":
                duration = cohort.get("elapsed_ns")
                samples = result["samples_ns"][index*measured:(index+1)*measured]
                if type(duration) is not int or duration < sum(samples):
                    raise RuntimeError("invalid action cohort elapsed time")
                elapsed += duration
        if role == "ping" and elapsed != result["elapsed_ns"]:
            raise RuntimeError("aggregate action elapsed time mismatch")
        wall = result.get("cohort_process_elapsed_ns")
        if type(wall) is not int or wall <= 0 or (role == "ping" and wall < elapsed):
            raise RuntimeError("invalid cohort process elapsed time")
        frames = [cohort.get("probe") for cohort in cohorts]
    present = [frame is not None for frame in frames]
    if any(present) and not all(present):
        raise RuntimeError("missing action probe frame")
    if instrumented is not None and all(present) != instrumented:
        raise RuntimeError("action instrumentation mode mismatch")
    for frame in frames:
        if frame is not None:
            validate_probe(frame, result["implementation"])
