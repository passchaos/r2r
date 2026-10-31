#!/usr/bin/env python3
"""Same-input baseline/candidate resource diagnostics, explicitly unqualified for timing."""
import gzip,hashlib,json,os,subprocess,sys,uuid
from pathlib import Path
ROOT=Path(__file__).resolve().parents[3]
sys.path.insert(0,str(ROOT/'tools/bench_ros'))
from callback_group_support.validation import validate
base=ROOT/'target/lazy-typesupport-comparison'
paths={'baseline':base/'r2r-baseline','candidate':ROOT/'target/callback-group-comparison/r2r-candidate'}
probe=ROOT/'target/callback-group-comparison/libr2r_bench_probe.so'
allowed=os.sched_getaffinity(0);cpus=[24,25,26,27]
os.sched_setaffinity(0,allowed-set(cpus))
env=os.environ.copy();env.update(RMW_IMPLEMENTATION='rmw_cyclonedds_cpp',ROS_DOMAIN_ID='182',ROS_LOCALHOST_ONLY='1',LD_PRELOAD=str(probe))
for key in ['CYCLONEDDS_URI','FASTRTPS_DEFAULT_PROFILES_FILE','FASTDDS_DEFAULT_PROFILES_FILE']:env.pop(key,None)
records=[]
for case in [('exclusive',2,4,0),('reentrant',4,8,16384)]:
 for repetition in range(4):
  for label in (['baseline','candidate'] if repetition%2==0 else ['candidate','baseline']):
   command=['taskset','-c',','.join(map(str,cpus)),str(paths[label]),*map(str,case),'256','32','/lazy_stage_'+uuid.uuid4().hex]
   process=subprocess.run(command,env=env,capture_output=True,text=True,timeout=60,check=True)
   result=json.loads(process.stdout);validate(result,'rust',case,256,32,env['RMW_IMPLEMENTATION'],True)
   records.append({'label':label,'case':case,'repetition':repetition,'result':result})
   print(label,case,repetition,'HWM KiB',result['process_hwm_kib'],flush=True)
report={'timing_qualified':False,'cpus':cpus,'binary_sha256':{label:hashlib.sha256(path.read_bytes()).hexdigest() for label,path in paths.items()},'probe_sha256':hashlib.sha256(probe.read_bytes()).hexdigest(),'records':records}
(base/'resource-stage.json.gz').write_bytes(gzip.compress(json.dumps(report,separators=(',',':')).encode(),mtime=0))
