import copy
import gzip
import hashlib
import json
import tempfile
import unittest
from pathlib import Path
from callback_group_support.validation import validate,summary,gate_failures,qualified_isolation,work_reference,PROBE_FIELDS
from callback_groups import cases_from,verify

CASE=('reentrant',2,2,0)

def sample(language='rust'):
    return {'process_threads':5,'process_rss_kib':900,'process_hwm_kib':1024,'implementation':'r2r' if language=='rust' else 'rclcpp','kind':'reentrant','workers':2,'launched_workers':2,'window':2,'operations':0,'iterations':2,'warmup':1,'rmw':'rmw_test','challenge':True,'challenge_overlap':True,'challenge_received':2,'challenge_maximum_active':2,'received':3,'checksum':3,'maximum_active':2,'elapsed_ns':50,'cpu_ns':30,'requests_per_second':40000000.,'rtt_ns_p50':30,'rtt_ns_p95':30,'rtt_ns_p99':30,'samples_ns':[30,30],'trace':[[1,2,2,10,15,25,40,work_reference(1,0)],[2,3,3,20,22,35,50,work_reference(2,0)]],'probe':None}

class Protocol(unittest.TestCase):
    def test_exact_trace(self):validate(sample(),'rust',CASE,2,1,'rmw_test')
    def test_reject_duplicates_missing_stage_and_identity(self):
        for mutate in [lambda r:r['trace'][1].__setitem__(0,1),lambda r:r['trace'][1].__setitem__(1,2),lambda r:r['trace'][0].__setitem__(3,9),lambda r:r['trace'][0].__setitem__(4,41),lambda r:r['trace'].pop(),lambda r:r.update(received=2),lambda r:r.update(checksum=2),lambda r:r['trace'][0].__setitem__(7,0)]:
            result=sample();mutate(result)
            with self.assertRaises(RuntimeError):validate(result,'rust',CASE,2,1,'rmw_test')
    def test_reject_missing_challenge_and_worker_peak(self):
        for changes in [{'challenge':False},{'challenge_overlap':False},{'maximum_active':1},{'maximum_active':3},{'launched_workers':3},{'workers':4}]:
            result=sample();result.update(changes)
            with self.assertRaises(RuntimeError):validate(result,'rust',CASE,2,1,'rmw_test')
    def test_reject_recomputed_bad_statistics_and_cropped_window(self):
        for changes in [{'samples_ns':[29,30]},{'rtt_ns_p95':29},{'requests_per_second':1.},{'cpu_ns':0},{'elapsed_ns':39}]:
            result=sample();result.update(changes)
            with self.assertRaises(RuntimeError):validate(result,'rust',CASE,2,1,'rmw_test')
    def test_exclusive_overlap_rejected(self):
        result=sample();result.update(kind='exclusive',maximum_active=1,challenge=False,challenge_overlap=False,challenge_received=0,challenge_maximum_active=0)
        with self.assertRaises(RuntimeError):validate(result,'rust',('exclusive',2,2,0),2,1,'rmw_test')
    def test_probe_required(self):
        with self.assertRaises(RuntimeError):validate(sample(),'rust',CASE,2,1,'rmw_test',True)
    def test_bools_are_not_sample_integers(self):
        result=sample();result['trace'][0][2]=True
        with self.assertRaises(RuntimeError):validate(result,'rust',CASE,2,1,'rmw_test')
    def test_matrix_must_be_complete(self):
        records=[{'case':list(CASE),'language':language,'repetition':0,'result':sample(language)} for language in ['rust','cpp']]
        for record in records:
            record['result']['probe']={field:0 for field in PROBE_FIELDS}
            record['result']['probe'].update(abi=1,malloc_calls=2,requested_bytes=64,user_cpu_ns=30,process_max_rss_kib=1024)
        rows=summary(records,[CASE],1);self.assertEqual(gate_failures(rows,[CASE],1),[])
        for invalid in [records[:1],records+records[:1]]:
            with self.assertRaises(RuntimeError):summary(invalid,[CASE],1)
        records[0]['result']['cpu_ns']=60
        self.assertTrue(gate_failures(summary(records,[CASE],1),[CASE],1.05))
    def test_invalid_cases(self):
        for value in ['reentrant:0:1:0','exclusive:2:0:0','bad:2:1:0','reentrant:2:1:-1','reentrant:2:1:0,reentrant:2:1:0','reentrant:2:1']:
            with self.assertRaises(ValueError):cases_from(value)

class Report(unittest.TestCase):
    def make(self,path):
        from callback_group_support.validation import PROBE_FIELDS
        from callback_groups import FORBIDDEN
        records=[];traces=[]
        for language in ['rust','cpp']:
            result=sample(language)
            result['probe']={field:0 for field in PROBE_FIELDS}
            result['probe'].update(abi=1,malloc_calls=2,requested_bytes=64,user_cpu_ns=30,process_max_rss_kib=1024)
            records.append({'case':list(CASE),'language':language,'repetition':0,'result':result})
        rows=summary(records,[CASE],1)
        for i,record in enumerate(records):
            result=record['result'];traces.append({'record':i,'trace':result.pop('trace'),'samples_ns':result.pop('samples_ns')})
        artifact=path.with_suffix('.gz');artifact.write_bytes(gzip.compress(json.dumps(traces).encode(),mtime=0))
        isolation=Isolation().valid();isolation['preflight']['forbidden_process_names']=list(FORBIDDEN)
        metadata={'workload':'same_node_callback_groups','cases':[list(CASE)],'iterations':2,'warmup':1,'repetitions':1,'cpus':[24,25],'rmw':'rmw_test','probe_enabled':True,'probe_sha256':'probe','binary_sha256':{'rust':'rust','cpp':'cpp'},'source_sha256':{},'source_mismatches':[],'build_manifest':{'workload':'callback_groups','candidate':{},'cpp_source_sha256':{},'binary_sha256':{'candidate':'rust','cpp':'cpp','probe':'probe'}},'trace_artifact':{'file':artifact.name,'sha256':hashlib.sha256(artifact.read_bytes()).hexdigest(),'records':2},'isolation':isolation}
        return {'metadata':metadata,'records':records,'summary':rows,'performance_gate':{'maximum_ratio':1.05,'failures':[],'passed':True}}
    def test_qualified_report(self):
        with tempfile.TemporaryDirectory() as directory:
            path=Path(directory)/'report.json';report=self.make(path);path.write_text(json.dumps(report));verify(path,True)
    def test_tampering_never_becomes_a_performance_pass(self):
        mutations=[lambda r:r['summary'][0].update(cpu_per_request_ns=0),lambda r:r['records'].reverse(),lambda r:r['metadata']['binary_sha256'].update(rust='wrong'),lambda r:r['metadata'].update(probe_sha256='wrong'),lambda r:r['metadata'].update(source_mismatches=['changed.rs']),lambda r:r['metadata']['isolation']['monitor'].update(passed=False)]
        for mutation in mutations:
            with tempfile.TemporaryDirectory() as directory:
                path=Path(directory)/'report.json';report=self.make(path);mutation(report);path.write_text(json.dumps(report))
                with self.assertRaises(RuntimeError):verify(path,True)
    def test_rehashed_wrong_response_trace_is_rejected(self):
        with tempfile.TemporaryDirectory() as directory:
            path=Path(directory)/'report.json';report=self.make(path);artifact=path.with_suffix('.gz')
            traces=json.loads(gzip.decompress(artifact.read_bytes()));traces[0]['trace'][0][7]=0
            artifact.write_bytes(gzip.compress(json.dumps(traces).encode(),mtime=0));report['metadata']['trace_artifact']['sha256']=hashlib.sha256(artifact.read_bytes()).hexdigest()
            path.write_text(json.dumps(report))
            with self.assertRaises(RuntimeError):verify(path,True)

class Isolation(unittest.TestCase):
    def valid(self):
        return {'preflight':{'passed':True,'required_consecutive_samples':3,'maximum_load1':2.,'interval_seconds':10.,'forbidden_process_names':['zig'],'samples':[{'observed_utc':f'2026-10-30T01:00:{s:02d}+00:00','load1':.1,'competing_processes':{}} for s in [0,10,20]]},'monitor':{'passed':True,'observation_count':2,'maximum_allowed_load1':2.,'maximum_load1':.2,'violations':[],'interval_seconds':.25}}
    def test_valid_evidence(self):self.assertTrue(qualified_isolation(self.valid(),['zig']))
    def test_false_green_preflight_rejected(self):
        for mutation in [lambda p:p.update(samples=[]),lambda p:p.update(required_consecutive_samples=1),lambda p:p.update(maximum_load1=20.),lambda p:p.update(forbidden_process_names=[]),lambda p:p['samples'][0].update(load1=3),lambda p:p['samples'][0].update(competing_processes={'zig':1}),lambda p:p['samples'][1].update(observed_utc='2026-10-30T01:00:00+00:00')]:
            data=self.valid();mutation(data['preflight'])
            with self.assertRaises(RuntimeError):qualified_isolation(data,['zig'])
    def test_false_green_monitor_rejected(self):
        for change in [{'observation_count':0},{'maximum_load1':3},{'violations':[{}]},{'interval_seconds':10},{'maximum_allowed_load1':20}]:
            data=self.valid();data['monitor'].update(change)
            with self.assertRaises(RuntimeError):qualified_isolation(data,['zig'])
    def test_failed_continuous_isolation_never_promotes(self):
        data=self.valid();data['monitor']['passed']=False
        self.assertFalse(qualified_isolation(data,['zig']))

if __name__=='__main__':unittest.main()
