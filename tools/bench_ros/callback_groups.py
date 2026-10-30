#!/usr/bin/env python3
"""Run independent interleaved same-Node Rust/rclcpp callback group applications."""
import argparse
import datetime
import gzip
import hashlib
import json
import math
import os
from pathlib import Path
import platform
import subprocess
import sys
import uuid
import xml.etree.ElementTree as ET
from callback_group_support.validation import validate,summary,gate_failures,qualified_isolation,work_reference
from pubsub_pipeline import DEFAULT_FORBIDDEN_PROCESS_NAMES,IsolationMonitor,wait_for_isolation

FORBIDDEN=(*DEFAULT_FORBIDDEN_PROCESS_NAMES,"lux-webgpu-comp")

ROOT=Path(__file__).resolve().parents[2]

def cases_from(value):
    cases=[]
    for item in value.split(','):
        fields=item.split(':')
        if len(fields)!=4:raise ValueError('case must be kind:workers:window:operations')
        case=(fields[0],*(int(v) for v in fields[1:]))
        if case in cases or case[0] not in ['exclusive','reentrant'] or not 1<=case[1]<=32 or not 1<=case[2]<=1024 or not 0<=case[3]<=1000000:
            raise ValueError('invalid/duplicate callback case')
        cases.append(case)
    if not cases:raise ValueError('empty callback cases')
    return cases

def digest(path):return hashlib.sha256(path.read_bytes()).hexdigest()

def source_hashes():
    paths=[ROOT/'r2r/benches/callback_groups.rs',ROOT/'r2r/benches/support/allocation_probe.rs',ROOT/'tools/bench_ros/callback_groups.py',ROOT/'tools/bench_ros/build.py',ROOT/'tools/bench_ros/callback_groups.cpp',ROOT/'tools/bench_ros/CMakeLists.txt',ROOT/'tools/bench_ros/codec_support/probe.hpp',ROOT/'tools/bench_ros/probe/probe.h',ROOT/'tools/bench_ros/pubsub_pipeline.py']
    paths.extend((ROOT/'r2r/benches/support/callback_groups').rglob('*.rs'))
    paths.extend((ROOT/'tools/bench_ros/callback_group_support').rglob('*.py'))
    paths.extend((ROOT/'tools/bench_ros/callback_group_support').rglob('*.hpp'))
    return {str(p.relative_to(ROOT)):digest(p) for p in sorted(set(paths))}

def verify(path,require_performance=False):
    report=json.loads(path.read_text());metadata=report['metadata']
    if metadata.get('workload')!='same_node_callback_groups':raise RuntimeError('invalid callback report workload')
    cases=[tuple(case) for case in metadata['cases']]
    if cases!=cases_from(','.join(':'.join(map(str,case)) for case in cases)):
        raise RuntimeError('invalid callback case metadata')
    if type(metadata['iterations']) is not int or metadata['iterations']<=0 or type(metadata['warmup']) is not int or metadata['warmup']<0 or metadata['iterations']+metadata['warmup']>1000000 or type(metadata['repetitions']) is not int or metadata['repetitions']<1:
        raise RuntimeError('invalid callback dimensions')
    cpus=metadata['cpus']
    if not isinstance(cpus,list) or not cpus or len(set(cpus))!=len(cpus) or any(type(cpu) is not int or cpu<0 for cpu in cpus):raise RuntimeError('invalid callback CPU affinity')
    artifact=metadata['trace_artifact'];data=(path.parent/artifact['file']).read_bytes()
    if digest(path.parent/artifact['file'])!=artifact['sha256']:raise RuntimeError('callback trace digest mismatch')
    traces=json.loads(gzip.decompress(data));records=report['records']
    if len(traces)!=len(records) or artifact['records']!=len(records) or any(trace['record']!=i for i,trace in enumerate(traces)):
        raise RuntimeError('callback trace reference mismatch')
    restored=[]
    for i,record in enumerate(records):
        result=dict(record['result']);result.update(samples_ns=traces[i]['samples_ns'],trace=traces[i]['trace'])
        validate(result,record['language'],tuple(record['case']),metadata['iterations'],metadata['warmup'],metadata['rmw'],metadata['probe_enabled'])
        restored.append({**record,'result':result})
    expected_order=[(list(case),language,repetition) for case in cases for repetition in range(metadata['repetitions']) for language in (['rust','cpp'] if repetition%2==0 else ['cpp','rust'])]
    if [(r['case'],r['language'],r['repetition']) for r in records]!=expected_order:raise RuntimeError('callback run order is not interleaved')
    build=metadata['build_manifest']
    if build.get('workload')!='callback_groups':raise RuntimeError('callback build manifest workload mismatch')
    for language,key in [('rust','candidate'),('cpp','cpp')]:
        if metadata['binary_sha256'][language]!=build.get('binary_sha256',{}).get(key):raise RuntimeError('callback binary/build provenance mismatch')
    if metadata['probe_enabled'] and metadata['probe_sha256']!=build.get('binary_sha256',{}).get('probe'):raise RuntimeError('callback probe/build provenance mismatch')
    for section in ['candidate','cpp_source_sha256']:
        for source,sha in build[section].items():
            if source in metadata['source_sha256'] and metadata['source_sha256'][source]!=sha:raise RuntimeError('callback source/build provenance mismatch')
    if report['summary']!=summary(restored,cases,metadata['repetitions']):raise RuntimeError('callback summary mismatch')
    gate=report.get('performance_gate')
    if gate:
        maximum=gate['maximum_ratio']
        if type(maximum) not in (float,int) or not math.isfinite(maximum) or maximum<=0:raise RuntimeError('invalid callback performance threshold')
        failures=gate_failures(report['summary'],cases,maximum)
        isolation=metadata.get('isolation',{})
        qualified=qualified_isolation(isolation,FORBIDDEN) and metadata.get('source_mismatches')==[]
        if gate['failures']!=failures or gate['passed'] is not (qualified and not failures):raise RuntimeError('callback performance gate mismatch')
    if require_performance and (not gate or not gate['passed']):raise RuntimeError('qualified passing callback performance gate required')
    print(f'{path}: {len(records)} callback applications, {sum(len(t["trace"]) for t in traces)} exact request traces verified')

def main():
    parser=argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--verify',type=Path)
    parser.add_argument('--require-performance-pass',action='store_true')
    parser.add_argument('--rust',type=Path);parser.add_argument('--cpp',type=Path)
    parser.add_argument('--probe',type=Path);parser.add_argument('--build-manifest',type=Path)
    parser.add_argument('--cpus',default='24,25,26,27')
    parser.add_argument('--cases',default='exclusive:2:4:0,reentrant:2:4:0,reentrant:2:4:16384,reentrant:4:8:16384')
    parser.add_argument('--iterations',type=int,default=1000);parser.add_argument('--warmup',type=int,default=100)
    parser.add_argument('--repetitions',type=int,default=5);parser.add_argument('--domain-id',type=int,default=182)
    parser.add_argument('--rmw',default='rmw_cyclonedds_cpp');parser.add_argument('--output',type=Path)
    parser.add_argument('--max-rclcpp-ratio',type=float)
    parser.add_argument('--isolation-samples',type=int,default=0);parser.add_argument('--isolation-interval',type=float,default=10)
    parser.add_argument('--isolation-timeout',type=float,default=50);parser.add_argument('--isolation-max-load1',type=float,default=2)
    args=parser.parse_args()
    if args.verify:verify(args.verify,args.require_performance_pass);return
    if not all([args.rust,args.cpp,args.output,args.build_manifest]):parser.error('run requires --rust, --cpp, --build-manifest and --output')
    try:cases=cases_from(args.cases);cpus=[int(value) for value in args.cpus.split(',')]
    except ValueError as error:parser.error(str(error))
    allowed=os.sched_getaffinity(0)
    if not cpus or len(set(cpus))!=len(cpus) or not set(cpus)<=allowed or not allowed-set(cpus):parser.error('measurement CPUs must be distinct and leave an orchestrator CPU')
    if args.iterations<=0 or args.warmup<0 or args.iterations+args.warmup>1000000 or args.repetitions<1 or not 0<=args.domain_id<=232:parser.error('invalid callback run dimensions')
    if args.isolation_samples<0 or any(not math.isfinite(v) or v<=0 for v in [args.isolation_interval,args.isolation_timeout,args.isolation_max_load1]):parser.error('invalid callback isolation settings')
    if args.max_rclcpp_ratio is not None and (not math.isfinite(args.max_rclcpp_ratio) or args.max_rclcpp_ratio<=0 or args.isolation_samples<3 or args.isolation_max_load1>2 or args.probe is None):parser.error('formal performance gate requires finite positive ratio and at least three idle samples')
    binaries={'rust':args.rust.resolve(),'cpp':args.cpp.resolve()}
    build=json.loads(args.build_manifest.read_text())
    if build.get('workload')!='callback_groups':parser.error('callback build manifest required')
    for relative,sha in build['candidate'].items():
        if digest(ROOT/relative)!=sha:parser.error(f'candidate source differs from its build: {relative}')
    for relative,sha in build['cpp_source_sha256'].items():
        if digest(ROOT/relative)!=sha:parser.error(f'C++ source differs from its build: {relative}')
    for language,key in [("rust","candidate"),("cpp","cpp")]:
        if digest(binaries[language])!=build.get("binary_sha256",{}).get(key):parser.error(f'{language} executable differs from the build manifest')
    if args.probe and digest(args.probe)!=build.get("binary_sha256",{}).get("probe"):parser.error('allocation probe differs from its build')
    os.sched_setaffinity(0,allowed-set(cpus))
    env=os.environ.copy();env.update(RMW_IMPLEMENTATION=args.rmw,ROS_DOMAIN_ID=str(args.domain_id),ROS_LOCALHOST_ONLY='1')
    for name in ['CYCLONEDDS_URI','FASTRTPS_DEFAULT_PROFILES_FILE','FASTDDS_DEFAULT_PROFILES_FILE']:env.pop(name,None)
    if args.probe:env['LD_PRELOAD']=str(args.probe.resolve())
    metadata={'started_utc':datetime.datetime.now(datetime.timezone.utc).isoformat(),'workload':'same_node_callback_groups','cases':[list(case) for case in cases],'iterations':args.iterations,'warmup':args.warmup,'repetitions':args.repetitions,'cpus':cpus,'domain_id':args.domain_id,'rmw':args.rmw,'ros_distro':env['ROS_DISTRO'],'platform':platform.platform(),'binary_sha256':{name:digest(binary) for name,binary in binaries.items()},'probe_enabled':bool(args.probe),'probe_sha256':digest(args.probe) if args.probe else None,'source_sha256':source_hashes(),'build_manifest':build,'rtt_boundary':'before native request send through application receipt; measured input queueing included','worker_model':'one application controller outside both fixed worker pools; exclusive launches min(budget,2), reentrant launches budget; one native Node/RMW service and client','qos':'reliable volatile KeepLast(max(16,window)), no intra-process path','trace_layout':['sequence','client_native_request_id','server_native_request_id','submit_ns','handler_enter_ns','handler_finish_ns','application_complete_ns','response_value']}
    metadata['cpu_model']=next(line.split(':',1)[1].strip() for line in Path('/proc/cpuinfo').read_text().splitlines() if line.startswith('model name'))
    metadata['rustc']=subprocess.check_output(['rustc','--version'],text=True).strip()
    metadata['cxx']=subprocess.check_output(['c++','--version'],text=True).splitlines()[0]
    prefix=Path('/opt/ros')/env['ROS_DISTRO']/'share'
    metadata['ros_packages']={name:ET.parse(prefix/name/'package.xml').findtext('version') for name in ['rcl','rclcpp','example_interfaces',args.rmw]}
    for operations in sorted({case[3] for case in cases}):
        if operations*args.iterations>100000000:parser.error('independent result oracle work exceeds 100 million steps per case')
        for sequence in range(args.warmup,args.warmup+args.iterations):work_reference(sequence,operations)
        print('independent result oracle prepared:',operations,flush=True)
    observations=[]
    def observe(names):
        from pubsub_pipeline import isolation_snapshot
        value=isolation_snapshot(names);observations.append(value);print(json.dumps(value),flush=True);return value
    try:
        preflight=wait_for_isolation(args.isolation_samples,args.isolation_interval,args.isolation_timeout,args.isolation_max_load1,FORBIDDEN,snapshot=observe) if args.isolation_samples else {'passed':False,'enabled':False}
    except RuntimeError as error:
        args.output.parent.mkdir(parents=True,exist_ok=True)
        args.output.write_text(json.dumps({'metadata':metadata,'isolation_failure':str(error),'observations':observations,'measurement_started':False},indent=2)+'\n');raise
    monitor=IsolationMonitor(FORBIDDEN,0.25,args.isolation_max_load1) if args.isolation_samples else None
    if monitor:monitor.start()
    records=[]
    try:
        for case in cases:
            for repetition in range(args.repetitions):
                for language in (['rust','cpp'] if repetition%2==0 else ['cpp','rust']):
                    name='/callback_comparison_'+uuid.uuid4().hex
                    command=['taskset','-c',','.join(map(str,cpus)),str(binaries[language]),*map(str,case),str(args.iterations),str(args.warmup),name]
                    result=subprocess.run(command,env=env,text=True,capture_output=True,timeout=120)
                    if result.returncode:raise RuntimeError(f'{language} callback process failed: {result.stderr}')
                    data=json.loads(result.stdout);validate(data,language,case,args.iterations,args.warmup,args.rmw,bool(args.probe))
                    records.append({'case':list(case),'language':language,'repetition':repetition,'service_name':name,'result':data})
                    print(language,case,repetition,'protocol pass',flush=True)
    except Exception as error:
        args.output.parent.mkdir(parents=True,exist_ok=True)
        args.output.with_suffix('.failure.json').write_text(json.dumps({'metadata':metadata,'records':records,'error':str(error),'last_stdout':result.stdout if 'result' in locals() else None,'last_stderr':result.stderr if 'result' in locals() else None},indent=2)+'\n')
        raise
    finally:
        if monitor:metadata['isolation']={'preflight':preflight,'monitor':monitor.finish()}
    expected_sources={**build['candidate'],**build['cpp_source_sha256'],**metadata['source_sha256']}
    metadata['source_mismatches']=[source for source,sha in expected_sources.items() if digest(ROOT/source)!=sha]
    rows=summary(records,cases,args.repetitions)
    traces=[]
    for i,record in enumerate(records):
        result=record['result'];traces.append({'record':i,'samples_ns':result.pop('samples_ns'),'trace':result.pop('trace')})
    args.output.parent.mkdir(parents=True,exist_ok=True)
    artifact=args.output.with_suffix('.traces.json.gz');artifact.write_bytes(gzip.compress(json.dumps(traces,separators=(',',':')).encode(),mtime=0))
    metadata['trace_artifact']={'file':artifact.name,'sha256':digest(artifact),'records':len(records)}
    report={'metadata':metadata,'records':records,'summary':rows}
    if args.max_rclcpp_ratio is not None:
        failures=gate_failures(rows,cases,args.max_rclcpp_ratio)
        report['performance_gate']={'maximum_ratio':args.max_rclcpp_ratio,'failures':failures,'passed':not failures and metadata['isolation']['monitor']['passed'] and not metadata['source_mismatches']}
    args.output.write_text(json.dumps(report,indent=2)+'\n')
    verify(args.output,args.require_performance_pass)
    if metadata['source_mismatches']:raise RuntimeError('callback sources changed during measurement; raw results retained')
    if report.get('performance_gate',{}).get('passed') is False:raise RuntimeError('callback performance gate failed; raw results retained')

if __name__=='__main__':main()
