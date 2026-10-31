import gzip
import hashlib
import json
import tempfile
import unittest
from pathlib import Path

from callback_group_startup import gate_failures,summary,verify
from callback_group_support.validation import work_reference
from callback_groups import FORBIDDEN

CASE=("exclusive",1,1,0)

def result(language):
    return {"process_threads":3,"process_rss_kib":900,"process_hwm_kib":1024,"implementation":"r2r" if language=="rust" else "rclcpp","kind":"exclusive","workers":1,"launched_workers":1,"window":1,"operations":0,"iterations":1,"warmup":0,"rmw":"rmw_test","challenge":False,"challenge_overlap":False,"challenge_received":0,"challenge_maximum_active":0,"received":1,"checksum":0,"maximum_active":1,"elapsed_ns":30,"cpu_ns":20,"requests_per_second":33333333.33333333,"rtt_ns_p50":30,"rtt_ns_p95":30,"rtt_ns_p99":30,"samples_ns":[30],"trace":[[0,1,1,10,15,25,40,work_reference(0,0)]],"probe":None}

def isolation():
    return {"preflight":{"passed":True,"required_consecutive_samples":3,"maximum_load1":2.,"interval_seconds":10.,"forbidden_process_names":list(FORBIDDEN),"samples":[{"observed_utc":f"2026-10-30T01:00:{second:02d}+00:00","load1":.1,"competing_processes":{}} for second in [0,10,20]]},"monitor":{"passed":True,"observation_count":2,"maximum_allowed_load1":2.,"maximum_load1":.2,"violations":[],"interval_seconds":.25}}

class StartupReport(unittest.TestCase):
    def make(self,path,repetitions=30):
        records=[]
        for repetition in range(repetitions):
            for language in (["rust","cpp"] if repetition%2==0 else ["cpp","rust"]):
                records.append({"language":language,"repetition":repetition,"service_name":f"/{language}_{repetition}","ready_wall_ns":50+repetition,"ready_from_main_ns":40+repetition,"process_wall_ns":100+repetition,"result":result(language)})
        rows=summary(records,repetitions);traces=[]
        for i,record in enumerate(records):
            value=record["result"];traces.append({"record":i,"samples_ns":value.pop("samples_ns"),"trace":value.pop("trace")})
        artifact=path.with_suffix(".traces.json.gz");artifact.write_bytes(gzip.compress(json.dumps(traces).encode(),mtime=0))
        metadata={"workload":"callback_group_cold_process","case":list(CASE),"iterations":1,"warmup":0,"repetitions":repetitions,"cpus":[24,25],"rmw":"rmw_test","binary_sha256":{"rust":"rust","cpp":"cpp"},"source_sha256":{},"source_mismatches":[],"build_manifest":{"workload":"callback_groups","candidate":{},"cpp_source_sha256":{},"binary_sha256":{"candidate":"rust","cpp":"cpp"}},"trace_artifact":{"file":artifact.name,"sha256":hashlib.sha256(artifact.read_bytes()).hexdigest(),"records":len(records)},"isolation":isolation()}
        return {"metadata":metadata,"records":records,"summary":rows,"performance_gate":{"maximum_ratio":1.05,"failures":[],"passed":True}}

    def test_qualified_report(self):
        with tempfile.TemporaryDirectory() as directory:
            path=Path(directory)/"startup.json";path.write_text(json.dumps(self.make(path)));verify(path,True)

    def test_tampering_never_becomes_a_performance_pass(self):
        mutations=[lambda report:report["records"][0].update(ready_wall_ns=20),lambda report:report["records"].reverse(),lambda report:report["summary"][0].update(ready_wall_ns_p50=0),lambda report:report["metadata"].update(iterations=2),lambda report:report["metadata"]["binary_sha256"].update(rust="wrong"),lambda report:report["metadata"].update(source_mismatches=["changed.py"]),lambda report:report["metadata"]["isolation"]["monitor"].update(passed=False)]
        for mutation in mutations:
            with tempfile.TemporaryDirectory() as directory:
                path=Path(directory)/"startup.json";report=self.make(path);mutation(report);path.write_text(json.dumps(report))
                with self.assertRaises(RuntimeError):verify(path,True)

    def test_rehashed_wrong_exchange_is_rejected(self):
        with tempfile.TemporaryDirectory() as directory:
            path=Path(directory)/"startup.json";report=self.make(path);artifact=path.with_suffix(".traces.json.gz")
            traces=json.loads(gzip.decompress(artifact.read_bytes()));traces[0]["trace"][0][7]=0
            artifact.write_bytes(gzip.compress(json.dumps(traces).encode(),mtime=0));report["metadata"]["trace_artifact"]["sha256"]=hashlib.sha256(artifact.read_bytes()).hexdigest();path.write_text(json.dumps(report))
            with self.assertRaises(RuntimeError):verify(path,True)

    def test_matrix_and_gate_reject_incomplete_or_slower_rust(self):
        records=[{"language":language,"repetition":0,"ready_wall_ns":50,"ready_from_main_ns":40,"process_wall_ns":100,"result":result(language)} for language in ["rust","cpp"]]
        rows=summary(records,1);self.assertEqual(gate_failures(rows,1),[])
        with self.assertRaises(RuntimeError):summary(records[:1],1)
        records[0]["ready_wall_ns"]=80;records[0]["process_wall_ns"]=200
        self.assertTrue(gate_failures(summary(records,1),1.05))

if __name__=="__main__":unittest.main()
