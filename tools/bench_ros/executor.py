#!/usr/bin/env python3
"""Compare complete multi-node service applications with native, historical and C++ executors."""
import argparse
import datetime
import gzip
import hashlib
import json
import math
import os
import platform
from pathlib import Path
import statistics
import subprocess
import uuid
import xml.etree.ElementTree as ET


FIELDS = ["rtt_ns_p50", "rtt_ns_p95", "rtt_ns_p99", "requests_per_second", "cpu_per_request_ns"]


def percentile(samples, percent):
    values = sorted(samples)
    return values[math.ceil((len(values)-1)*percent/100)]


def validate(result, language, nodes, window, iterations, warmup, rmw, baseline_shared=False, control_enabled=False):
    driver = {"rust":"shared_wait", "cpp":"single_threaded_executor", "baseline":"shared_wait" if baseline_shared else "node_wait" if nodes==1 else "nonblocking_round_robin"}[language]
    if (result["implementation"],result["driver"],result["nodes"],result["window"],result["iterations"],result["warmup"],result["rmw"]) != ("rclcpp" if language=="cpp" else "r2r",driver,nodes,window,iterations,warmup,rmw):
        raise RuntimeError("executor identity or driver mismatch")
    if result.get("control_enabled",False) is not (control_enabled and language=="rust"):
        raise RuntimeError("executor cancellation control mode mismatch")
    total = iterations+warmup
    if result["received"] != total or result["checksum"] != total*(total-1)//2:
        raise RuntimeError("executor request count/checksum mismatch")
    samples=result["samples_ns"]
    if len(samples)!=iterations or any(type(value) is not int or value<=0 for value in samples):
        raise RuntimeError("invalid executor RTT samples")
    if result["elapsed_ns"] < (sum(samples) if window==1 else max(samples)) or result["cpu_ns"]<=0:
        raise RuntimeError("invalid executor wall/CPU window")
    for percent in [50,95,99]:
        if result[f"rtt_ns_p{percent}"] != percentile(samples,percent):
            raise RuntimeError("executor percentile mismatch")
    expected=iterations/(result["elapsed_ns"]*1e-9)
    if not math.isfinite(result["requests_per_second"]) or not math.isclose(result["requests_per_second"],expected,rel_tol=1e-5):
        raise RuntimeError("executor throughput mismatch")


def regressions(summary, maximum):
    failures=[]
    for candidate in [row for row in summary if row["language"]=="rust"]:
        baseline=next(row for row in summary if (row["nodes"],row["window"],row["language"])==(candidate["nodes"],candidate["window"],"baseline"))
        ratios={field:candidate[field]/baseline[field] for field in FIELDS if field!="requests_per_second"}
        ratios["inverse_throughput"]=baseline["requests_per_second"]/candidate["requests_per_second"]
        if any(ratio>maximum for ratio in ratios.values()):
            failures.append({"nodes":candidate["nodes"],"window":candidate["window"],"ratios":ratios})
    return failures


def verify(path, require_performance_pass=False):
    report=json.loads(path.read_text())
    metadata=report["metadata"]
    artifact=metadata["samples_artifact"]
    compressed=(path.parent/artifact["file"]).read_bytes()
    if hashlib.sha256(compressed).hexdigest()!=artifact["sha256"]:
        raise RuntimeError("executor sample digest mismatch")
    traces=json.loads(gzip.decompress(compressed))
    records=report["records"]
    if len(traces)!=len(records) or artifact["records"]!=len(records) or {trace["record"] for trace in traces}!=set(range(len(records))):
        raise RuntimeError("executor sample references mismatch")
    keys=set()
    for trace in traces:
        record=dict(records[trace["record"]])
        key=(record["nodes"],record["window"],record["language"],record["repetition"])
        if key in keys: raise RuntimeError("duplicate executor record")
        keys.add(key)
        record["samples_ns"]=trace["samples_ns"]
        validate(record,record["language"],record["nodes"],record["window"],metadata["iterations"],metadata["warmup"],metadata["rmw"],metadata.get("baseline_shared",False),metadata.get("control_enabled",False))
        if record["cpu_per_request_ns"] != record["cpu_ns"]/metadata["iterations"]:
            raise RuntimeError("executor CPU calculation mismatch")
    expected={(nodes,window,language,repetition) for nodes in metadata["nodes"] for window in metadata["windows"] for language in ["baseline","rust","cpp"] for repetition in range(metadata["repetitions"])}
    if keys!=expected: raise RuntimeError("incomplete executor application matrix")
    summary_keys=set()
    for row in report["summary"]:
        key=(row["nodes"],row["window"],row["language"])
        if key in summary_keys: raise RuntimeError("duplicate executor summary")
        summary_keys.add(key)
        group=[record for record in records if (record["nodes"],record["window"],record["language"])==key]
        if len(group)!=metadata["repetitions"]: raise RuntimeError("incomplete executor summary")
        for field in FIELDS:
            if row[field]!=statistics.median(record[field] for record in group):
                raise RuntimeError("executor summary median mismatch")
    if summary_keys!={(nodes,window,language) for nodes in metadata["nodes"] for window in metadata["windows"] for language in ["baseline","rust","cpp"]}:
        raise RuntimeError("incomplete executor summary matrix")
    gate=metadata.get("regression_gate")
    if gate:
        failures=regressions(report["summary"],gate["maximum_ratio"])
        if gate["passed"]!=(not failures) or gate["failures"]!=failures:
            raise RuntimeError("executor regression gate mismatch")
    if require_performance_pass and (not gate or not gate["passed"]):
        raise RuntimeError("passing executor regression gate required")
    print(f"{path}: {len(records)} application records and {sum(len(trace['samples_ns']) for trace in traces)} RTT samples verified")


def run(args):
    nodes=[int(value) for value in args.nodes.split(',')]
    windows=[int(value) for value in args.windows.split(',')]
    if not nodes or len(set(nodes))!=len(nodes) or any(not 1<=value<=64 for value in nodes):
        raise ValueError("invalid executor node counts")
    if not windows or len(set(windows))!=len(windows) or any(not 1<=value<=1024 for value in windows):
        raise ValueError("invalid request windows")
    if args.iterations<=0 or args.warmup<0 or args.repetitions<1 or args.iterations+args.warmup<max(nodes) or args.iterations+args.warmup>2**31-1 or args.cpu not in os.sched_getaffinity(0) or not 0<=args.domain_id<=232:
        raise ValueError("invalid executor dimensions or CPU/domain")
    if args.max_regression is not None and (not math.isfinite(args.max_regression) or args.max_regression<1):
        raise ValueError("invalid executor regression tolerance")
    binaries={"baseline":args.baseline.resolve(),"rust":args.rust.resolve(),"cpp":args.cpp.resolve()}
    env=os.environ.copy()
    env.update(RMW_IMPLEMENTATION=args.rmw,ROS_DOMAIN_ID=str(args.domain_id),ROS_LOCALHOST_ONLY="1")
    for name in ["CYCLONEDDS_URI","FASTRTPS_DEFAULT_PROFILES_FILE","FASTDDS_DEFAULT_PROFILES_FILE"]: env.pop(name,None)
    metadata={"workload":"multi_node_service","started_utc":datetime.datetime.now(datetime.timezone.utc).isoformat(),
              "cpu":args.cpu,"domain_id":args.domain_id,"rmw":args.rmw,"ros_distro":env["ROS_DISTRO"],
              "platform":platform.platform(),
              "cpu_model":next(line.split(':',1)[1].strip() for line in Path('/proc/cpuinfo').read_text().splitlines() if line.startswith('model name')),
              "nodes":nodes,"windows":windows,"iterations":args.iterations,"warmup":args.warmup,"repetitions":args.repetitions,
              "baseline_shared":args.baseline_shared,
              "control_enabled":args.control_enabled,
              "git_head":subprocess.check_output(["git","rev-parse","HEAD"],text=True).strip(),
              "working_tree_dirty":bool(subprocess.check_output(["git","status","--porcelain"],text=True).strip()),
              "binary_sha256":{name:hashlib.sha256(path.read_bytes()).hexdigest() for name,path in binaries.items()},
              "rustc":subprocess.check_output(["rustc","--version"],text=True).strip(),
              "cxx":subprocess.check_output(["c++","--version"],text=True).splitlines()[0],
              "qos":"ServicesQoS reliable volatile KeepLast(max(16,window)); intra-process disabled",
              "scheduling":"one application thread and all RMW threads on the same fixed CPU; candidate shared native wait versus " + ("baseline shared native wait" if args.baseline_shared else "historical native single-node wait/nonblocking multi-node round robin") + " and rclcpp SingleThreadedExecutor",
              "rtt_boundary":"before async request through response retrieval, before sum validation; queueing inside the window included",
              "throughput_boundary":"complete measured requests including validation; excludes setup/discovery/warmup/teardown",
              "cpu_boundary":"process CPU clock across all threads inside the measured window, per completed request",
              "source_sha256":{str(path):hashlib.sha256(path.read_bytes()).hexdigest() for path in [Path('r2r/benches/multi_node_service.rs'),Path('tools/bench_ros/multi_node_service.cpp'),Path('tools/bench_ros/executor.py')]}}
    if args.build_manifest: metadata["build_manifest"]=json.loads(args.build_manifest.read_text())
    prefix=Path('/opt/ros')/env['ROS_DISTRO']/'share'
    metadata['ros_packages']={name:ET.parse(prefix/name/'package.xml').findtext('version') for name in ['rcl','rclcpp','example_interfaces',args.rmw]}
    records,traces=[],[]
    for count in nodes:
        for window in windows:
            for repetition in range(args.repetitions):
                for language in (["baseline","rust","cpp"] if repetition%2==0 else ["cpp","rust","baseline"]):
                    command=["taskset","-c",str(args.cpu),str(binaries[language]),str(count),str(window),str(args.iterations),str(args.warmup),"/executor_bench_"+uuid.uuid4().hex]
                    run_env=env.copy()
                    if args.control_enabled and language=="rust": run_env["R2R_BENCH_CONTROL_ENABLED"]="1"
                    else: run_env.pop("R2R_BENCH_CONTROL_ENABLED",None)
                    result=subprocess.run(command,env=run_env,text=True,capture_output=True,timeout=120)
                    if result.returncode: raise RuntimeError(f"{language} executor failed: {result.stderr}")
                    record=json.loads(result.stdout)
                    validate(record,language,count,window,args.iterations,args.warmup,args.rmw,args.baseline_shared,args.control_enabled)
                    traces.append({"record":len(records),"samples_ns":record.pop("samples_ns")})
                    record.update(language=language,repetition=repetition,cpu_per_request_ns=record["cpu_ns"]/args.iterations)
                    records.append(record)
                    print(f"{language} nodes={count} window={window} repetition={repetition+1}: p50={record['rtt_ns_p50']} p99={record['rtt_ns_p99']} CPU/request={record['cpu_per_request_ns']:.1f} ns",flush=True)
    summary=[]
    for count in nodes:
        for window in windows:
            for language in ["baseline","rust","cpp"]:
                group=[record for record in records if (record["nodes"],record["window"],record["language"])==(count,window,language)]
                summary.append({"nodes":count,"window":window,"language":language,**{field:statistics.median(record[field] for record in group) for field in FIELDS}})
    args.output.parent.mkdir(parents=True,exist_ok=True)
    compressed=gzip.compress(json.dumps(traces,separators=(',',':')).encode(),mtime=0)
    artifact=args.output.with_suffix('.samples.json.gz')
    artifact.write_bytes(compressed)
    metadata["samples_artifact"]={"file":artifact.name,"sha256":hashlib.sha256(compressed).hexdigest(),"records":len(records)}
    metadata["completed_utc"]=datetime.datetime.now(datetime.timezone.utc).isoformat()
    failures=regressions(summary,args.max_regression) if args.max_regression is not None else []
    if args.max_regression is not None: metadata["regression_gate"]={"maximum_ratio":args.max_regression,"passed":not failures,"failures":failures}
    args.output.write_text(json.dumps({"metadata":metadata,"records":records,"summary":summary},indent=2)+'\n')
    if failures: raise RuntimeError(f"executor regression gate failed: {failures}")


def main():
    parser=argparse.ArgumentParser(description=__doc__)
    commands=parser.add_subparsers(dest='command',required=True)
    runner=commands.add_parser('run')
    for name in ['baseline','rust','cpp']: runner.add_argument('--'+name,type=Path,required=True)
    runner.add_argument('--cpu',type=int,required=True)
    runner.add_argument('--nodes',default='1,2,8')
    runner.add_argument('--windows',default='1,16')
    runner.add_argument('--iterations',type=int,default=5000)
    runner.add_argument('--warmup',type=int,default=100)
    runner.add_argument('--repetitions',type=int,default=5)
    runner.add_argument('--domain-id',type=int,default=210)
    runner.add_argument('--rmw',default='rmw_cyclonedds_cpp')
    runner.add_argument('--max-regression',type=float)
    runner.add_argument('--baseline-shared',action='store_true',help='baseline library already provides the shared native executor')
    runner.add_argument('--control-enabled',action='store_true',help='candidate explicitly creates and retains its cancellation handle before measurement')
    runner.add_argument('--build-manifest',type=Path)
    runner.add_argument('--output',type=Path,required=True)
    checker=commands.add_parser('verify')
    checker.add_argument('reports',nargs='+',type=Path)
    checker.add_argument('--require-performance-pass',action='store_true')
    args=parser.parse_args()
    if args.command=='run': run(args)
    else:
        for path in args.reports: verify(path,args.require_performance_pass)


if __name__=='__main__': main()
