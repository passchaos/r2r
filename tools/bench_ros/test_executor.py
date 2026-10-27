import copy
import gzip
import hashlib
import json
from pathlib import Path
import tempfile
import unittest

from executor import regressions, validate, verify


class ExecutorProtocolValidation(unittest.TestCase):
    def setUp(self):
        self.record={"implementation":"r2r","driver":"shared_wait","nodes":1,"window":1,
                     "iterations":2,"warmup":1,"received":3,"checksum":3,"rmw":"rmw_cyclonedds_cpp",
                     "samples_ns":[10,20],"elapsed_ns":40,"cpu_ns":25,"requests_per_second":50_000_000,
                     "rtt_ns_p50":20,"rtt_ns_p95":20,"rtt_ns_p99":20}

    def check(self,record):
        validate(record,"rust",1,1,2,1,"rmw_cyclonedds_cpp")

    def test_full_protocol_oracle_passes(self):
        self.check(self.record)

    def test_wrong_driver_counts_or_clock_windows_fail(self):
        for field,value in [("driver","nonblocking_round_robin"),("received",2),("checksum",4),
                            ("nodes",2),("window",16),("rmw","wrong"),("samples_ns",[0,20]),
                            ("rtt_ns_p99",10),("elapsed_ns",20),("cpu_ns",0),
                            ("requests_per_second",float('nan'))]:
            record=dict(self.record)
            record[field]=value
            with self.subTest(field=field),self.assertRaises(RuntimeError): self.check(record)

    def test_cpu_and_tail_regressions_are_independent_gates(self):
        baseline={"language":"baseline","nodes":1,"window":1,"rtt_ns_p50":10,"rtt_ns_p95":20,
                  "rtt_ns_p99":30,"requests_per_second":100,"cpu_per_request_ns":50}
        candidate=dict(baseline,language="rust")
        self.assertFalse(regressions([baseline,candidate],1.05))
        for field in ['cpu_per_request_ns','rtt_ns_p99']:
            changed=dict(candidate)
            changed[field]*=1.2
            self.assertTrue(regressions([baseline,changed],1.05))

    def test_archived_matrix_requires_all_languages_and_exact_samples(self):
        with tempfile.TemporaryDirectory() as directory:
            path=Path(directory)/'report.json'
            records=[]
            traces=[]
            summary=[]
            for language,driver in [('baseline','node_wait'),('rust','shared_wait'),('cpp','single_threaded_executor')]:
                record=dict(self.record,language=language,repetition=0,driver=driver,
                            implementation='rclcpp' if language=='cpp' else 'r2r',cpu_per_request_ns=12.5)
                traces.append({'record':len(records),'samples_ns':record.pop('samples_ns')})
                records.append(record)
                summary.append({key:record[key] for key in ['language','nodes','window','rtt_ns_p50','rtt_ns_p95','rtt_ns_p99','requests_per_second','cpu_per_request_ns']})
            compressed=gzip.compress(json.dumps(traces).encode(),mtime=0)
            (path.parent/'samples.gz').write_bytes(compressed)
            report={'metadata':{'iterations':2,'warmup':1,'repetitions':1,'nodes':[1],'windows':[1],
                                'rmw':'rmw_cyclonedds_cpp','samples_artifact':{'file':'samples.gz','sha256':hashlib.sha256(compressed).hexdigest(),'records':3},
                                'regression_gate':{'maximum_ratio':1.05,'passed':True,'failures':[]}},'records':records,'summary':summary}
            path.write_text(json.dumps(report))
            verify(path,True)
            corrupt=copy.deepcopy(report)
            corrupt['records'][0]['driver']='shared_wait'
            path.write_text(json.dumps(corrupt))
            with self.assertRaisesRegex(RuntimeError,'driver mismatch'): verify(path)
            path.write_text(json.dumps(report))
            (path.parent/'samples.gz').write_bytes(b'corrupt')
            with self.assertRaisesRegex(RuntimeError,'digest'): verify(path)
