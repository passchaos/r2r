import json
from pathlib import Path
import tempfile
import unittest

from verify_codec import verify


class CodecArchiveValidation(unittest.TestCase):
    def setUp(self):
        self.directory = tempfile.TemporaryDirectory()
        self.addCleanup(self.directory.cleanup)
        self.path = Path(self.directory.name) / "report.json"
        self.report = {"metadata": {"rmw": "rmw_cyclonedds_cpp", "payload_sizes": [32],
                                   "modes": ["serialize_reuse"], "repetitions": 1,
                                   "binary_sha256": {"baseline": "a", "candidate": "b", "rclcpp": "c"},
                                   "regression_gate": {"maximum_ratio": 1.05, "passed": True, "failures": []}},
                       "records": [], "summary": [{"elements": 32, "mode": "serialize_reuse",
                                   "median_ns_per_op": {"baseline": 12.0, "candidate": 12.0, "rclcpp": 12.0},
                                   "relative_to_rclcpp": {"baseline": 1.0, "candidate": 1.0, "rclcpp": 1.0}}]}
        for label in ["baseline", "candidate", "rclcpp"]:
            self.report["records"].append({"label": label, "implementation": "rclcpp" if label == "rclcpp" else "r2r",
                "rmw": "rmw_cyclonedds_cpp", "mode": "serialize_reuse", "elements": 32, "iterations": 1000,
                "repetition": 0, "wire_bytes": 172, "wire_hash": 42, "checksum": 172000,
                "elapsed_ns": 12000, "ns_per_op": 12.0,
                "batch_ns_per_op_p50": 12.0, "batch_ns_per_op_p95": 12.0, "batch_ns_per_op_p99": 12.0})

    def check(self):
        self.path.write_text(json.dumps(self.report))
        verify(self.path, require_performance_pass=True)

    def test_complete_matrix_passes(self):
        self.check()

    def test_archived_string_fixture_cannot_use_integer_records(self):
        self.report["metadata"]["fixture"]="string_array"
        with self.assertRaisesRegex(RuntimeError,"fixture mismatch"):
            self.check()
        for row in self.report["records"]:
            row["fixture"]="string_array"
        self.check()
        self.report["records"][0]["fixture"]="int32_array"
        with self.assertRaisesRegex(RuntimeError,"fixture mismatch"):
            self.check()

    def test_missing_record_fails(self):
        self.report["records"].pop()
        with self.assertRaisesRegex(RuntimeError, "matrix"):
            self.check()

    def test_wrong_fixture_checksum_and_summary_fail(self):
        for field, value in [("wire_hash", 43), ("checksum", 1), ("ns_per_op", 13.0)]:
            row = self.report["records"][0]
            saved = row[field]
            row[field] = value
            with self.subTest(field=field), self.assertRaises(RuntimeError):
                self.check()
            row[field] = saved

    def test_gate_cannot_claim_false_success(self):
        self.report["metadata"]["regression_gate"]["passed"] = False
        with self.assertRaisesRegex(RuntimeError, "gate"):
            self.check()

    def test_instrumentation_cannot_claim_a_latency_gate(self):
        self.report["metadata"]["allocation_probe"]={"sha256":"abc"}
        with self.assertRaisesRegex(RuntimeError,"cannot claim a latency gate"):
            self.check()


if __name__ == "__main__":
    unittest.main()
