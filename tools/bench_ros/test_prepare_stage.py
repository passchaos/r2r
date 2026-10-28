import copy
import gzip
import hashlib
import json
from pathlib import Path
import tempfile
import unittest

from prepare_stage import BOUNDARY, regression, summarize, verify


class NativeStageIntegrity(unittest.TestCase):
    def fixture(self, folder):
        samples = [{"servers": 1, "label": label, "repetition": 0, "samples_ns": [10, 20, 30]} for label in ["baseline", "candidate"]]
        artifact = folder / "samples.json.gz"
        artifact.write_bytes(gzip.compress(json.dumps(samples).encode(), mtime=0))
        records = [{"servers": 1, "label": label, "repetition": 0, "iterations": 3, "warmup": 1, "loop_ns": 90, "ns_per_call": 30.0, "p50_ns": 20, "p95_ns": 30, "p99_ns": 30, "boundary": BOUNDARY, "rmw_identifier": "rmw_cyclonedds_cpp", "cpu_affinity": "24", "maintenance_polls": 3 if label == "baseline" else 0} for label in ["baseline", "candidate"]]
        summary = summarize(records)
        return {"metadata": {"boundary": BOUNDARY, "rmw": "rmw_cyclonedds_cpp", "cpu": 24, "servers": [1], "iterations": 3, "warmup": 1, "repetitions": 1, "samples": {"file": artifact.name, "sha256": hashlib.sha256(artifact.read_bytes()).hexdigest()}}, "records": records, "summary": summary, "gate": regression(summary, 1.05)}

    def test_native_counts_identity_and_sample_percentiles_are_required(self):
        with tempfile.TemporaryDirectory() as directory:
            folder = Path(directory)
            report = self.fixture(folder)
            path = folder / "report.json"
            path.write_text(json.dumps(report))
            verify(path)
            for field, value in [("maintenance_polls", 0), ("rmw_identifier", "other"), ("cpu_affinity", "24-25"), ("p99_ns", 29), ("loop_ns", 50)]:
                changed = copy.deepcopy(report)
                changed["records"][0][field] = value
                path.write_text(json.dumps(changed))
                with self.assertRaises(AssertionError):
                    verify(path)
            artifact = folder / report["metadata"]["samples"]["file"]
            artifact.write_bytes(artifact.read_bytes() + b"tampered")
            path.write_text(json.dumps(report))
            with self.assertRaises(AssertionError):
                verify(path)

    def test_gate_rejects_nonfinite_or_lowered_tolerance(self):
        for maximum in [float("nan"), float("inf"), 0.99]:
            with self.assertRaises(AssertionError):
                regression([], maximum)


if __name__ == "__main__":
    unittest.main()
