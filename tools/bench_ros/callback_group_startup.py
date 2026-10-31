#!/usr/bin/env python3
"""Measure a cold callback-group process through one verified service exchange."""
import argparse
import datetime
import gzip
import json
import math
import os
from pathlib import Path
import platform
import selectors
import statistics
import subprocess
import sys
import time
import uuid
import xml.etree.ElementTree as ET

from callback_groups import FORBIDDEN,cases_from,digest,source_hashes as callback_source_hashes
from callback_group_support.validation import percentile,qualified_isolation,validate
from pubsub_pipeline import IsolationMonitor,wait_for_isolation

ROOT=Path(__file__).resolve().parents[2]
GATE_FIELDS=("ready_wall_ns_p50","ready_wall_ns_p95","ready_wall_ns_p99","process_wall_ns_p50","process_wall_ns_p95","process_wall_ns_p99","process_hwm_kib","process_threads")

def parse_case(value):
    cases=cases_from(value)
    if len(cases)!=1:raise ValueError("startup protocol accepts exactly one callback case")
    return cases[0]

def source_hashes():
    sources=callback_source_hashes()
    path=Path(__file__).resolve()
    sources[str(path.relative_to(ROOT))]=digest(path)
    return dict(sorted(sources.items()))

def summary(records,repetitions):
    expected={(language,repetition) for language in ["rust","cpp"] for repetition in range(repetitions)}
    seen=set()
    for record in records:
        key=(record.get("language"),record.get("repetition"))
        if key in seen:raise RuntimeError("duplicate cold-process run")
        seen.add(key)
    if seen!=expected:raise RuntimeError("incomplete cold-process comparison matrix")
    rows=[]
    for language in ["rust","cpp"]:
        group=[record for record in records if record["language"]==language]
        ready=[record["ready_wall_ns"] for record in group]
        main_ready=[record["ready_from_main_ns"] for record in group]
        walls=[record["process_wall_ns"] for record in group]
        exchanges=[record["result"]["elapsed_ns"] for record in group]
        after_ready=[wall-ready_wall for wall,ready_wall in zip(walls,ready)]
        row={"language":language}
        for percent in [50,95,99]:
            row[f"ready_wall_ns_p{percent}"]=percentile(ready,percent)
            row[f"ready_from_main_ns_p{percent}"]=percentile(main_ready,percent)
            row[f"process_wall_ns_p{percent}"]=percentile(walls,percent)
            row[f"exchange_ns_p{percent}"]=percentile(exchanges,percent)
            row[f"after_ready_ns_p{percent}"]=percentile(after_ready,percent)
        row["process_hwm_kib"]=statistics.median(record["result"]["process_hwm_kib"] for record in group)
        row["process_threads"]=statistics.median(record["result"]["process_threads"] for record in group)
        rows.append(row)
    return rows

def gate_failures(rows,maximum):
    rust=next(row for row in rows if row["language"]=="rust")
    cpp=next(row for row in rows if row["language"]=="cpp")
    ratios={field:(rust[field]/cpp[field] if cpp[field]>0 else 1. if rust[field]==0 else math.inf) for field in GATE_FIELDS}
    return [] if all(ratio<=maximum for ratio in ratios.values()) else [{"ratios":ratios}]

def verify(path,require_performance=False):
    report=json.loads(path.read_text());metadata=report["metadata"]
    if metadata.get("workload")!="callback_group_cold_process":raise RuntimeError("invalid startup report workload")
    if metadata.get("iterations")!=1 or metadata.get("warmup")!=0:raise RuntimeError("invalid startup request dimensions")
    case=tuple(metadata.get("case",[]))
    if case!=parse_case(":".join(map(str,case))):raise RuntimeError("invalid startup callback case")
    repetitions=metadata.get("repetitions")
    if type(repetitions) is not int or repetitions<1:raise RuntimeError("invalid startup repetition count")
    cpus=metadata.get("cpus")
    if not isinstance(cpus,list) or not cpus or len(set(cpus))!=len(cpus) or any(type(cpu) is not int or cpu<0 for cpu in cpus):raise RuntimeError("invalid startup CPU affinity")
    artifact=metadata["trace_artifact"];artifact_path=path.parent/artifact["file"]
    if digest(artifact_path)!=artifact["sha256"]:raise RuntimeError("startup trace digest mismatch")
    traces=json.loads(gzip.decompress(artifact_path.read_bytes()));records=report["records"]
    if len(traces)!=len(records) or artifact["records"]!=len(records) or any(trace["record"]!=i for i,trace in enumerate(traces)):
        raise RuntimeError("startup trace reference mismatch")
    restored=[]
    for i,record in enumerate(records):
        result=dict(record["result"]);result.update(samples_ns=traces[i]["samples_ns"],trace=traces[i]["trace"])
        validate(result,record["language"],case,1,0,metadata["rmw"],False)
        wall=record.get("process_wall_ns");ready=record.get("ready_wall_ns");main_ready=record.get("ready_from_main_ns")
        if any(type(value) is not int for value in [wall,ready,main_ready]) or not 0<main_ready<=ready<wall or wall-ready<=result["elapsed_ns"]:
            raise RuntimeError("invalid cold-process readiness/wall interval")
        restored.append({**record,"result":result})
    expected=[(language,repetition) for repetition in range(repetitions) for language in (["rust","cpp"] if repetition%2==0 else ["cpp","rust"])]
    if [(record["language"],record["repetition"]) for record in records]!=expected:raise RuntimeError("startup run order is not interleaved")
    build=metadata["build_manifest"]
    if build.get("workload")!="callback_groups":raise RuntimeError("callback build manifest required")
    for language,key in [("rust","candidate"),("cpp","cpp")]:
        if metadata["binary_sha256"][language]!=build.get("binary_sha256",{}).get(key):raise RuntimeError("startup binary/build provenance mismatch")
    for section in ["candidate","cpp_source_sha256"]:
        for source,sha in build[section].items():
            if source in metadata["source_sha256"] and metadata["source_sha256"][source]!=sha:raise RuntimeError("startup source/build provenance mismatch")
    rows=summary(restored,repetitions)
    if report["summary"]!=rows:raise RuntimeError("startup summary mismatch")
    gate=report.get("performance_gate")
    if gate:
        maximum=gate["maximum_ratio"]
        if type(maximum) not in (float,int) or not math.isfinite(maximum) or maximum<=0 or repetitions<30:raise RuntimeError("invalid startup performance threshold")
        failures=gate_failures(rows,maximum)
        qualified=qualified_isolation(metadata.get("isolation",{}),FORBIDDEN) and metadata.get("source_mismatches")==[]
        if gate["failures"]!=failures or gate["passed"] is not (qualified and not failures):raise RuntimeError("startup performance gate mismatch")
    if require_performance and (not gate or not gate["passed"]):raise RuntimeError("qualified passing startup gate required")
    print(f"{path}: {len(records)} cold processes and {len(traces)} exact exchanges verified")

def main():
    parser=argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--verify",type=Path);parser.add_argument("--require-performance-pass",action="store_true")
    parser.add_argument("--rust",type=Path);parser.add_argument("--cpp",type=Path);parser.add_argument("--build-manifest",type=Path)
    parser.add_argument("--cpus",default="24,25,26,27");parser.add_argument("--case",default="exclusive:1:1:0")
    parser.add_argument("--repetitions",type=int,default=10);parser.add_argument("--domain-id",type=int,default=182)
    parser.add_argument("--rmw",default="rmw_cyclonedds_cpp");parser.add_argument("--output",type=Path)
    parser.add_argument("--process-timeout",type=float,default=30)
    parser.add_argument("--max-rclcpp-ratio",type=float)
    parser.add_argument("--isolation-samples",type=int,default=0);parser.add_argument("--isolation-interval",type=float,default=10)
    parser.add_argument("--isolation-timeout",type=float,default=50);parser.add_argument("--isolation-max-load1",type=float,default=2)
    args=parser.parse_args()
    if args.verify:verify(args.verify,args.require_performance_pass);return
    if not all([args.rust,args.cpp,args.output,args.build_manifest]):parser.error("run requires --rust, --cpp, --build-manifest and --output")
    try:case=parse_case(args.case);cpus=[int(value) for value in args.cpus.split(",")]
    except ValueError as error:parser.error(str(error))
    allowed=os.sched_getaffinity(0)
    if not cpus or len(set(cpus))!=len(cpus) or not set(cpus)<=allowed or not allowed-set(cpus):parser.error("measurement CPUs must be distinct and leave an orchestrator CPU")
    if args.repetitions<1 or not 0<=args.domain_id<=232 or not math.isfinite(args.process_timeout) or args.process_timeout<=0:parser.error("invalid startup dimensions")
    if args.isolation_samples<0 or any(not math.isfinite(value) or value<=0 for value in [args.isolation_interval,args.isolation_timeout,args.isolation_max_load1]):parser.error("invalid startup isolation settings")
    if args.max_rclcpp_ratio is not None and (not math.isfinite(args.max_rclcpp_ratio) or args.max_rclcpp_ratio<=0 or args.repetitions<30 or args.isolation_samples<3 or args.isolation_max_load1>2):parser.error("formal startup gate requires 30 repetitions and three idle samples")
    binaries={"rust":args.rust.resolve(),"cpp":args.cpp.resolve()};build=json.loads(args.build_manifest.read_text())
    if build.get("workload")!="callback_groups":parser.error("callback build manifest required")
    for relative,sha in build["candidate"].items():
        if digest(ROOT/relative)!=sha:parser.error(f"candidate source differs from its build: {relative}")
    for relative,sha in build["cpp_source_sha256"].items():
        if digest(ROOT/relative)!=sha:parser.error(f"C++ source differs from its build: {relative}")
    for language,key in [("rust","candidate"),("cpp","cpp")]:
        if digest(binaries[language])!=build.get("binary_sha256",{}).get(key):parser.error(f"{language} executable differs from the build manifest")
    os.sched_setaffinity(0,allowed-set(cpus))
    env=os.environ.copy();env.update(RMW_IMPLEMENTATION=args.rmw,ROS_DOMAIN_ID=str(args.domain_id),ROS_LOCALHOST_ONLY="1",R2R_BENCH_STARTUP_MARKER="1")
    for name in ["CYCLONEDDS_URI","FASTRTPS_DEFAULT_PROFILES_FILE","FASTDDS_DEFAULT_PROFILES_FILE","LD_PRELOAD"]:env.pop(name,None)
    runtime_sources=source_hashes()
    metadata={"started_utc":datetime.datetime.now(datetime.timezone.utc).isoformat(),"workload":"callback_group_cold_process","case":list(case),"iterations":1,"warmup":0,"repetitions":args.repetitions,"cpus":cpus,"domain_id":args.domain_id,"rmw":args.rmw,"ros_distro":env["ROS_DISTRO"],"platform":platform.platform(),"binary_sha256":{name:digest(binary) for name,binary in binaries.items()},"source_sha256":runtime_sources,"build_manifest":build,"ready_boundary":"parent monotonic time immediately before process creation through the flushed ready marker after ROS/RMW initialization and self-service discovery; includes dynamic loading and language runtime entry","process_boundary":"same start through one exact service exchange, output, shutdown and successful process exit","main_ready_boundary":"binary monotonic time near main entry through the ready marker; excludes executable loading","exchange_boundary":"binary monotonic time immediately before its one request through validated application receipt","qos":"reliable volatile KeepLast(16), no intra-process path"}
    metadata["cpu_model"]=next(line.split(":",1)[1].strip() for line in Path("/proc/cpuinfo").read_text().splitlines() if line.startswith("model name"))
    metadata["rustc"]=subprocess.check_output(["rustc","--version"],text=True).strip();metadata["cxx"]=subprocess.check_output(["c++","--version"],text=True).splitlines()[0]
    prefix=Path("/opt/ros")/env["ROS_DISTRO"]/"share"
    metadata["ros_packages"]={name:ET.parse(prefix/name/"package.xml").findtext("version") for name in ["rcl","rclcpp","example_interfaces",args.rmw]}
    observations=[]
    def observe(names):
        from pubsub_pipeline import isolation_snapshot
        value=isolation_snapshot(names);observations.append(value);print(json.dumps(value),flush=True);return value
    try:
        preflight=wait_for_isolation(args.isolation_samples,args.isolation_interval,args.isolation_timeout,args.isolation_max_load1,FORBIDDEN,snapshot=observe) if args.isolation_samples else {"passed":False,"enabled":False}
    except RuntimeError as error:
        args.output.parent.mkdir(parents=True,exist_ok=True);args.output.write_text(json.dumps({"metadata":metadata,"isolation_failure":str(error),"observations":observations,"measurement_started":False},indent=2)+"\n");raise
    monitor=IsolationMonitor(FORBIDDEN,.25,args.isolation_max_load1) if args.isolation_samples else None
    if monitor:monitor.start()
    records=[]
    try:
        for repetition in range(args.repetitions):
            for language in (["rust","cpp"] if repetition%2==0 else ["cpp","rust"]):
                name="/callback_startup_"+uuid.uuid4().hex
                command=["taskset","-c",",".join(map(str,cpus)),str(binaries[language]),*map(str,case),"1","0",name]
                before=time.monotonic_ns();process=subprocess.Popen(command,env=env,text=True,stdout=subprocess.PIPE,stderr=subprocess.PIPE,bufsize=1)
                selector=selectors.DefaultSelector();selector.register(process.stdout,selectors.EVENT_READ);deadline=time.monotonic()+args.process_timeout
                try:
                    if not selector.select(max(0,deadline-time.monotonic())):raise RuntimeError(f"{language} ready marker timeout")
                    marker_line=process.stdout.readline();ready_wall=time.monotonic_ns()-before
                    stdout,stderr=process.communicate(timeout=max(.001,deadline-time.monotonic()))
                except Exception:
                    process.kill();process.communicate();raise
                finally:selector.close()
                wall=time.monotonic_ns()-before
                if process.returncode:raise RuntimeError(f"{language} cold process failed: {stderr}")
                marker=json.loads(marker_line);expected_implementation="r2r" if language=="rust" else "rclcpp"
                if marker.get("event")!="ready" or marker.get("implementation")!=expected_implementation or type(marker.get("ready_from_main_ns")) is not int:
                    raise RuntimeError(f"{language} invalid ready marker")
                result=json.loads(stdout);validate(result,language,case,1,0,args.rmw,False)
                if not 0<marker["ready_from_main_ns"]<=ready_wall<wall or wall-ready_wall<=result["elapsed_ns"]:raise RuntimeError("cold-process intervals do not contain their phases")
                records.append({"language":language,"repetition":repetition,"service_name":name,"ready_wall_ns":ready_wall,"ready_from_main_ns":marker["ready_from_main_ns"],"process_wall_ns":wall,"result":result})
                print(language,repetition,"cold-process protocol pass",flush=True)
    except Exception as error:
        args.output.parent.mkdir(parents=True,exist_ok=True);args.output.with_suffix(".failure.json").write_text(json.dumps({"metadata":metadata,"records":records,"error":str(error)},indent=2)+"\n");raise
    finally:
        if monitor:metadata["isolation"]={"preflight":preflight,"monitor":monitor.finish()}
    expected_sources={**build["candidate"],**build["cpp_source_sha256"],**runtime_sources}
    metadata["source_mismatches"]=[source for source,sha in expected_sources.items() if digest(ROOT/source)!=sha]
    rows=summary(records,args.repetitions);traces=[]
    for i,record in enumerate(records):
        result=record["result"];traces.append({"record":i,"samples_ns":result.pop("samples_ns"),"trace":result.pop("trace")})
    args.output.parent.mkdir(parents=True,exist_ok=True);artifact=args.output.with_suffix(".traces.json.gz");artifact.write_bytes(gzip.compress(json.dumps(traces,separators=(",",":")).encode(),mtime=0))
    metadata["trace_artifact"]={"file":artifact.name,"sha256":digest(artifact),"records":len(records)}
    report={"metadata":metadata,"records":records,"summary":rows}
    if args.max_rclcpp_ratio is not None:
        failures=gate_failures(rows,args.max_rclcpp_ratio);qualified=qualified_isolation(metadata["isolation"],FORBIDDEN) and not metadata["source_mismatches"]
        report["performance_gate"]={"maximum_ratio":args.max_rclcpp_ratio,"failures":failures,"passed":qualified and not failures}
    args.output.write_text(json.dumps(report,indent=2)+"\n");verify(args.output,args.require_performance_pass)
    if metadata["source_mismatches"]:raise RuntimeError("startup sources changed during measurement; raw results retained")
    if report.get("performance_gate",{}).get("passed") is False:raise RuntimeError("startup performance gate failed; raw results retained")

if __name__=="__main__":main()
