#!/usr/bin/env python3
"""Measure/verify foreign-future notification latency using independent Rust binaries."""
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


def percentile(samples, percent):
    return sorted(samples)[math.ceil((len(samples)-1)*percent/100)]


def validate(row, rmw, iterations, warmup, delay):
    if (row['implementation'],row['rmw'],row['iterations'],row['warmup'],row['delay_us']) != ('r2r',rmw,iterations,warmup,delay):
        raise RuntimeError('future benchmark identity mismatch')
    total=iterations+warmup
    if row['completed']!=total or row['checksum']!=total*(total-1)//2:
        raise RuntimeError('future completion count/checksum mismatch')
    samples=row['samples_ns']
    if len(samples)!=iterations or any(type(value) is not int or value<=0 for value in samples):
        raise RuntimeError('invalid future notification samples')
    for percent in [50,95,99]:
        if row[f'notification_ns_p{percent}']!=percentile(samples,percent):
            raise RuntimeError('future percentile mismatch')
    if type(row['elapsed_ns']) is not int or row['elapsed_ns']<sum(samples):
        raise RuntimeError('invalid future elapsed time')
    if not math.isclose(row['completions_per_second'],iterations/(row['elapsed_ns']*1e-9),rel_tol=1e-10):
        raise RuntimeError('future completion rate mismatch')


FIELDS=['notification_ns_p50','notification_ns_p95','notification_ns_p99','completions_per_second']
def gate(summary, maximum):
    if not math.isfinite(maximum) or maximum<1:raise RuntimeError('invalid future regression limit')
    baseline=next(row for row in summary if row['language']=='baseline')
    candidate=next(row for row in summary if row['language']=='candidate')
    ratios={name:candidate[name]/baseline[name] for name in FIELDS[:3]}
    ratios['inverse_throughput']=baseline['completions_per_second']/candidate['completions_per_second']
    return {'maximum_ratio':maximum,'ratios':ratios,'passed':all(value<=maximum for value in ratios.values())}


def verify(path, require_pass=False):
    report=json.loads(path.read_text());meta=report['metadata']
    artifact=meta['samples_artifact'];compressed=(path.parent/artifact['file']).read_bytes()
    if hashlib.sha256(compressed).hexdigest()!=artifact['sha256']:raise RuntimeError('future sample digest mismatch')
    samples=json.loads(gzip.decompress(compressed))
    records=report['records']
    if len(samples)!=len(records) or len(records)!=2*meta['repetitions']:raise RuntimeError('future matrix incomplete')
    keys=set()
    for index,record in enumerate(records):
        key=(record['language'],record['repetition'])
        if key in keys:raise RuntimeError('duplicate future record')
        keys.add(key)
        row=dict(record['result'],samples_ns=samples[index])
        validate(row,meta['rmw'],meta['iterations'],meta['warmup'],meta['delay_us'])
    if keys!={(language,repetition) for language in ['baseline','candidate'] for repetition in range(meta['repetitions'])}:
        raise RuntimeError('future matrix incomplete')
    expected=[{'language':language,**{field:statistics.median(row['result'][field] for row in records if row['language']==language) for field in FIELDS}} for language in ['baseline','candidate']]
    if report['summary']!=expected:raise RuntimeError('future summary mismatch')
    if meta['regression_gate']!=gate(expected,meta['regression_gate']['maximum_ratio']):raise RuntimeError('future gate mismatch')
    if require_pass and not meta['regression_gate']['passed']:raise RuntimeError('future performance gate failed')
    print(f'{path}: {len(records)} records, {sum(map(len,samples))} notification samples verified')


def run(args):
    allowed=os.sched_getaffinity(0)
    if args.cpu not in allowed or not 0<=args.domain_id<=232 or args.iterations<=0 or args.warmup<0 or args.iterations+args.warmup>2**32-1 or args.repetitions<1 or not 0<args.delay_us<1_000_000 or not math.isfinite(args.max_regression) or args.max_regression<1:
        raise ValueError('invalid future benchmark configuration')
    env=os.environ.copy();env.pop('CYCLONEDDS_URI',None)
    env.update(RMW_IMPLEMENTATION=args.rmw,ROS_DOMAIN_ID=str(args.domain_id))
    binaries={'baseline':args.baseline.resolve(),'candidate':args.candidate.resolve()}
    records,samples=[],[]
    meta={'started_utc':datetime.datetime.now(datetime.timezone.utc).isoformat(),'rmw':args.rmw,'ros_distro':env['ROS_DISTRO'],
          'cpu':args.cpu,'domain_id':args.domain_id,'iterations':args.iterations,'warmup':args.warmup,'delay_us':args.delay_us,'repetitions':args.repetitions,
          'platform':platform.platform(),'rustc':subprocess.check_output(['rustc','--version'],text=True).strip(),
          'binary_sha256':{name:hashlib.sha256(path.read_bytes()).hexdigest() for name,path in binaries.items()},
          'boundary':'external producer timestamp before oneshot send through Node::spin_until_future_complete return; producer sleep excluded; completion rate includes producer delay',
          'build_manifest':json.loads(args.build_manifest.read_text())}
    for repetition in range(args.repetitions):
        for language in ['baseline','candidate'] if repetition%2==0 else ['candidate','baseline']:
            result=subprocess.run(['taskset','-c',str(args.cpu),str(binaries[language]),str(args.iterations),str(args.warmup),str(args.delay_us)],env=env,capture_output=True,text=True,timeout=120,check=True)
            row=json.loads(result.stdout);validate(row,args.rmw,args.iterations,args.warmup,args.delay_us)
            samples.append(row.pop('samples_ns'));records.append({'language':language,'repetition':repetition,'result':row})
            print(language,repetition+1,'p50/p99 ns',row['notification_ns_p50'],row['notification_ns_p99'],flush=True)
    summary=[{'language':language,**{field:statistics.median(row['result'][field] for row in records if row['language']==language) for field in FIELDS}} for language in ['baseline','candidate']]
    args.output.parent.mkdir(parents=True,exist_ok=True)
    sample_path=args.output.with_suffix('.samples.json.gz');compressed=gzip.compress(json.dumps(samples,separators=(',',':')).encode(),mtime=0);sample_path.write_bytes(compressed)
    meta.update(completed_utc=datetime.datetime.now(datetime.timezone.utc).isoformat(),samples_artifact={'file':sample_path.name,'sha256':hashlib.sha256(compressed).hexdigest()},regression_gate=gate(summary,args.max_regression))
    args.output.write_text(json.dumps({'metadata':meta,'records':records,'summary':summary},indent=2)+'\n')
    verify(args.output,True)


def main():
    parser=argparse.ArgumentParser(description=__doc__);sub=parser.add_subparsers(dest='command',required=True)
    command=sub.add_parser('run')
    for name in ['baseline','candidate','build-manifest','output']:command.add_argument('--'+name,type=Path,required=True)
    command.add_argument('--cpu',type=int,required=True);command.add_argument('--domain-id',type=int,default=201)
    command.add_argument('--rmw',default='rmw_cyclonedds_cpp');command.add_argument('--iterations',type=int,default=200)
    command.add_argument('--warmup',type=int,default=10);command.add_argument('--delay-us',type=int,default=1000)
    command.add_argument('--repetitions',type=int,default=5);command.add_argument('--max-regression',type=float,default=1.05)
    command=sub.add_parser('verify');command.add_argument('reports',type=Path,nargs='+');command.add_argument('--require-performance-pass',action='store_true')
    args=parser.parse_args()
    if args.command=='run':run(args)
    else:
        for path in args.reports:verify(path,args.require_performance_pass)


if __name__=='__main__':main()
