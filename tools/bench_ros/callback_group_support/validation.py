"""Strict protocol and phase validation for same-Node callback group applications."""
import math
import statistics
from functools import lru_cache

FIELDS=("rtt_ns_p50","rtt_ns_p95","rtt_ns_p99","requests_per_second","cpu_per_request_ns","allocations_per_request","requested_bytes_per_request","process_hwm_kib","process_threads")
PROBE_FIELDS=("malloc_calls","calloc_calls","realloc_calls","aligned_calls","free_calls","null_free_calls","failed_allocations","requested_bytes","size_overflows","counter_overflows","user_cpu_ns","system_cpu_ns","voluntary_context_switches","involuntary_context_switches","process_max_rss_kib")

@lru_cache(maxsize=65536)
def work_reference(sequence,operations):
    mask=(1<<64)-1
    value=(sequence+0x9e3779b97f4a7c15)&mask
    for step in range(operations):
        value=(value^(value<<13))&mask
        value^=value>>7
        value=(value^(value<<17))&mask
        value=(value+step+0x517cc1b727220a95)&mask
    return value&((1<<63)-1)

def percentile(samples, percent):
    values=sorted(samples)
    return values[math.ceil((len(values)-1)*percent/100)]

def validate(result, language, case, iterations, warmup, rmw, require_probe=False):
    kind,workers,window,operations=case
    dimensions=["workers","launched_workers","process_threads","process_rss_kib","process_hwm_kib","window","operations","iterations","warmup","received","checksum","challenge_received","challenge_maximum_active"]
    if any(type(result.get(field)) is not int for field in dimensions):raise RuntimeError("invalid callback integer dimensions")

    if (result.get("implementation"),result.get("kind"),result.get("workers"),result.get("window"),result.get("operations"),result.get("iterations"),result.get("warmup"),result.get("rmw")) != ("r2r" if language=="rust" else "rclcpp",*case,iterations,warmup,rmw):
        raise RuntimeError("callback workload identity mismatch")
    limit=1 if kind=="exclusive" else workers
    if result.get("launched_workers")!=min(workers,1+limit):
        raise RuntimeError("native/callback worker budget mismatch")
    if result["process_threads"]<result["launched_workers"]+1:raise RuntimeError("missing callback controller/worker threads")
    if not 0<result["process_rss_kib"]<=result["process_hwm_kib"]:raise RuntimeError("invalid callback application memory snapshot")
    challenge=kind=="reentrant" and workers>=2 and window>=2
    if result.get("challenge") is not challenge or result.get("challenge_overlap") is not challenge:
        raise RuntimeError("same-source overlap challenge missing")
    if (result["challenge_received"],result["challenge_maximum_active"])!=((2,2) if challenge else (0,0)):raise RuntimeError("callback overlap challenge counts mismatch")
    total=iterations+warmup
    if result.get("received")!=total or result.get("checksum")!=total*(total-1)//2:
        raise RuntimeError("callback request count/checksum mismatch")
    if type(result.get("maximum_active")) is not int or not 1<=result["maximum_active"]<=min(limit,window):
        raise RuntimeError("callback invocation bound mismatch")
    trace=result.get("trace")
    samples=result.get("samples_ns")
    if not isinstance(trace,list) or not isinstance(samples,list) or len(trace)!=iterations or len(samples)!=iterations:
        raise RuntimeError("incomplete callback request trace")
    native=set()
    for position,row in enumerate(trace):
        if not isinstance(row,list) or len(row)!=8 or any(type(v) is not int for v in row):
            raise RuntimeError("invalid callback stage trace")
        sequence,request_id,server_id,submit,enter,finish,complete,result_value=row
        if sequence!=warmup+position or request_id in native:
            raise RuntimeError("duplicate/missing sequence or native request identity")
        native.add(request_id)
        if request_id!=server_id:raise RuntimeError("callback client/server native identity mismatch")
        if result_value!=work_reference(sequence,operations):raise RuntimeError("callback independent result oracle mismatch")
        if not 0<submit<=enter<=finish<complete:
            raise RuntimeError("callback stage ordering mismatch")
        if type(samples[position]) is not int or samples[position]!=complete-submit:
            raise RuntimeError("callback RTT/stage trace mismatch")
    # Sweep request and handler intervals independently. Equal endpoints retire
    # before admission, so back-to-back exclusive calls do not count as overlap.
    for start,end,bound,label in [(3,6,window,"request window"),(4,5,limit,"handler overlap")]:
        active=0;peak=0
        events=sorted((value,delta) for row in trace for value,delta in [(row[start],1),(row[end],-1)])
        for _,delta in events:
            active+=delta;peak=max(peak,active)
        if active or peak>bound:
            raise RuntimeError(f"callback {label} exceeds configured bound")
        if label=="handler overlap" and peak>result["maximum_active"]:
            raise RuntimeError("callback reported active peak contradicts the trace")
    elapsed=result.get("elapsed_ns");cpu=result.get("cpu_ns");rate=result.get("requests_per_second")
    if type(elapsed) is not int or elapsed<max(samples) or elapsed<max(row[6] for row in trace)-min(row[3] for row in trace) or type(cpu) is not int or cpu<=0:
        raise RuntimeError("invalid callback wall/CPU window")
    if type(rate) not in (float,int) or not math.isfinite(rate) or not math.isclose(rate,iterations/(elapsed*1e-9),rel_tol=1e-5):
        raise RuntimeError("callback throughput calculation mismatch")
    for percent in [50,95,99]:
        if result.get(f"rtt_ns_p{percent}")!=percentile(samples,percent):
            raise RuntimeError("callback percentile mismatch")
    probe=result.get("probe")
    if require_probe and probe is None:raise RuntimeError("complete-process allocation/resource probe missing")
    if probe is not None:
        if not isinstance(probe,dict) or (type(probe.get("abi")) is not int or probe["abi"]!=1) or any(type(probe.get(f)) is not int or probe[f]<0 for f in PROBE_FIELDS):
            raise RuntimeError("invalid callback process probe")
        if any(probe[f] for f in ["failed_allocations","size_overflows","counter_overflows"]):
            raise RuntimeError("callback process probe failure/overflow")
        if probe["process_max_rss_kib"]<=0 or probe["user_cpu_ns"]+probe["system_cpu_ns"]<=0:
            raise RuntimeError("invalid callback resource window")

def summary(records,cases,repetitions):
    rows=[]
    expected={(tuple(case),language,repetition) for case in cases for language in ["rust","cpp"] for repetition in range(repetitions)}
    seen=set()
    for record in records:
        key=(tuple(record["case"]),record["language"],record["repetition"])
        if key in seen:raise RuntimeError("duplicate callback run")
        seen.add(key)
    if seen!=expected:raise RuntimeError("incomplete callback comparison matrix")
    for case in cases:
        for language in ["rust","cpp"]:
            group=[record["result"] for record in records if record["case"]==list(case) and record["language"]==language]
            row={"case":list(case),"language":language}
            def metric(r,field):
                if field=="cpu_per_request_ns":return r["cpu_ns"]/r["iterations"]
                if field=="allocations_per_request":return None if r["probe"] is None else sum(r["probe"][key] for key in ["malloc_calls","calloc_calls","realloc_calls","aligned_calls"])/r["iterations"]
                if field=="requested_bytes_per_request":return None if r["probe"] is None else r["probe"]["requested_bytes"]/r["iterations"]
                return r[field]
            for field in FIELDS:
                values=[metric(r,field) for r in group]
                if all(value is None for value in values):row[field]=None
                elif any(value is None for value in values):raise RuntimeError("inconsistent callback resource probe coverage")
                else:row[field]=statistics.median(values)
            rows.append(row)
    return rows

def gate_failures(rows,cases,maximum):
    failures=[]
    for case in cases:
        rust=next(row for row in rows if row["case"]==list(case) and row["language"]=="rust")
        cpp=next(row for row in rows if row["case"]==list(case) and row["language"]=="cpp")
        if any(rust[f] is None or cpp[f] is None for f in FIELDS):raise RuntimeError("callback performance gate requires complete resource evidence")
        ratios={f:(rust[f]/cpp[f] if cpp[f]>0 else 1. if rust[f]==0 else math.inf) for f in FIELDS if f!="requests_per_second"}
        ratios["inverse_throughput"]=cpp["requests_per_second"]/rust["requests_per_second"]
        if any(value>maximum for value in ratios.values()):failures.append({"case":list(case),"ratios":ratios})
    return failures

def qualified_isolation(isolation, required_names):
    """Validate actual isolation evidence before accepting a performance claim."""
    from datetime import datetime
    preflight=isolation.get('preflight',{})
    monitor=isolation.get('monitor',{})
    if preflight.get('passed') is not True:return False
    count=preflight.get('required_consecutive_samples')
    maximum=preflight.get('maximum_load1')
    interval=preflight.get('interval_seconds')
    samples=preflight.get('samples')
    if (type(count) is not int or count<3 or type(maximum) not in (int,float)
        or not math.isfinite(maximum) or not 0<maximum<=2 or type(interval) not in (int,float)
        or not math.isfinite(interval) or interval<=0 or not isinstance(samples,list) or len(samples)!=count
        or not set(required_names)<=set(preflight.get('forbidden_process_names',[]))):
        raise RuntimeError('invalid callback isolation preflight evidence')
    times=[]
    for sample in samples:
        load=sample.get('load1')
        if type(load) not in (int,float) or not math.isfinite(load) or not 0<=load<=maximum or sample.get('competing_processes')!={}:
            raise RuntimeError('non-idle callback isolation sample')
        times.append(datetime.fromisoformat(sample['observed_utc']))
    if any((b-a).total_seconds()<interval*.9 for a,b in zip(times,times[1:])):
        raise RuntimeError('callback isolation samples are not independently spaced')
    if monitor.get('passed') is not True:return False
    if (type(monitor.get('observation_count')) is not int or monitor['observation_count']<2
        or monitor.get('maximum_allowed_load1')!=maximum
        or type(monitor.get('maximum_load1')) not in (float,int)
        or not math.isfinite(monitor['maximum_load1']) or not 0<=monitor['maximum_load1']<=maximum
        or monitor.get('violations')!=[] or type(monitor.get('interval_seconds')) not in (float,int)
        or not 0<monitor['interval_seconds']<=.25):
        raise RuntimeError('invalid callback continuous isolation evidence')
    return True
