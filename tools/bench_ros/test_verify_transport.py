import gzip
import copy
import hashlib
import json
from pathlib import Path
import tempfile
import unittest

from verify_transport import verify
from transport import regression_failures


class ArchivedMatrixValidation(unittest.TestCase):
    def setUp(self):
        self.directory = tempfile.TemporaryDirectory()
        self.addCleanup(self.directory.cleanup)
        self.path = Path(self.directory.name) / "report.json"
        self.records, self.traces, self.summary = [], [], []
        for ping in ["rust", "cpp"]:
            for echo in ["rust", "cpp"]:
                endpoint = {"role": "echo", "rmw": "rmw_cyclonedds_cpp", "elements": 0,
                            "iterations": 2, "warmup": 0, "received": 2, "checksum": 1}
                initiator = dict(endpoint, role="ping", implementation="r2r" if ping == "rust" else "rclcpp",
                                 elapsed_ns=40, roundtrips_per_second=50_000_000.0, rtt_ns_p50=20,
                                 rtt_ns_p95=20, rtt_ns_p99=20, rtt_ns_max=20)
                responder = dict(endpoint, implementation="r2r" if echo == "rust" else "rclcpp")
                self.traces.append({"record": len(self.records), "samples_ns": [10, 20]})
                self.records.append({"elements": 0, "ping_language": ping, "echo_language": echo,
                                     "repetition": 0, "ping": initiator, "echo": responder})
                self.summary.append({"elements": 0, "ping_language": ping, "echo_language": echo,
                                     "rtt_ns_p50": 20, "rtt_ns_p95": 20, "rtt_ns_p99": 20,
                                     "roundtrips_per_second": 50_000_000.0})

    def write(self):
        compressed = gzip.compress(json.dumps(self.traces).encode(), mtime=0)
        self.path.with_name("samples.gz").write_bytes(compressed)
        metadata = {"rmw": "rmw_cyclonedds_cpp", "iterations": 2, "warmup": 0, "repetitions": 1,
                    "payload_sizes": [0], "samples_artifact": {"file": "samples.gz",
                    "sha256": hashlib.sha256(compressed).hexdigest(), "record_count": len(self.records)}}
        if any(record["ping_language"] == "baseline" for record in self.records):
            metadata["binary_sha256"] = {"baseline": "test_binary"}
        self.path.write_text(json.dumps({"metadata": metadata, "records": self.records, "summary": self.summary}))

    def test_complete_matrix_passes(self):
        self.write()
        verify(self.path)

    def test_missing_language_combination_fails(self):
        self.records.pop()
        self.traces.pop()
        self.summary.pop()
        self.write()
        with self.assertRaisesRegex(RuntimeError, "incomplete"):
            verify(self.path)

    def test_corrupt_sample_artifact_fails(self):
        self.write()
        self.path.with_name("samples.gz").write_bytes(b"corrupt")
        with self.assertRaisesRegex(RuntimeError, "digest"):
            verify(self.path)

    def test_incorrect_summary_fails(self):
        self.summary[0]["rtt_ns_p99"] = 10
        self.write()
        with self.assertRaisesRegex(RuntimeError, "median"):
            verify(self.path)

    def test_service_matrix_checks_window_and_overlapping_latencies(self):
        for record in self.records:
            record["window"] = 16
            record.pop("elements")
            for role in ["ping", "echo"]:
                record[role]["window"] = 16
                record[role].pop("elements")
            record["ping"]["elapsed_ns"] = 25
            record["ping"]["roundtrips_per_second"] = 80_000_000.0
        for row in self.summary:
            row["window"] = 16
            row.pop("elements")
            row["roundtrips_per_second"] = 80_000_000.0
        self.write()
        report = json.loads(self.path.read_text())
        report["metadata"].update(workload="service", request_windows=[16])
        report["metadata"].pop("payload_sizes")
        self.path.write_text(json.dumps(report))
        verify(self.path)
        report["metadata"]["request_windows"] = [1]
        self.path.write_text(json.dumps(report))
        with self.assertRaisesRegex(RuntimeError, "incomplete"):
            verify(self.path)

    def test_baseline_matrix_requires_all_seven_combinations(self):
        for ping, echo in [("baseline", "baseline"), ("baseline", "cpp"), ("cpp", "baseline")]:
            record = copy.deepcopy(self.records[0])
            record.update(ping_language=ping, echo_language=echo)
            for role, language in [("ping", ping), ("echo", echo)]:
                record[role]["implementation"] = "rclcpp" if language == "cpp" else "r2r"
            row = dict(self.summary[0], ping_language=ping, echo_language=echo)
            self.traces.append({"record": len(self.records), "samples_ns": [10, 20]})
            self.records.append(record)
            self.summary.append(row)
        self.write()
        verify(self.path)
        self.records.pop()
        self.traces.pop()
        self.summary.pop()
        self.write()
        with self.assertRaisesRegex(RuntimeError, "incomplete"):
            verify(self.path)

    def test_missing_performance_gate_cannot_be_claimed_as_a_pass(self):
        self.write()
        with self.assertRaisesRegex(RuntimeError, "passing performance"):
            verify(self.path, require_performance_pass=True)

    def test_recorded_gate_must_agree_with_raw_measurements(self):
        for ping, echo in [("baseline", "baseline"), ("baseline", "cpp"), ("cpp", "baseline")]:
            record = copy.deepcopy(self.records[0])
            record.update(ping_language=ping, echo_language=echo)
            for role, language in [("ping", ping), ("echo", echo)]:
                record[role]["implementation"] = "rclcpp" if language == "cpp" else "r2r"
            self.traces.append({"record": len(self.records), "samples_ns": [10, 20]})
            self.records.append(record)
            self.summary.append(dict(self.summary[0], ping_language=ping, echo_language=echo))
        self.write()
        report = json.loads(self.path.read_text())
        report["metadata"]["regression_gate"] = {
            "maximum_ratio": 1.05, "passed": True,
            "failures": regression_failures(self.summary, [0], 1.05),
        }
        self.path.write_text(json.dumps(report))
        verify(self.path, require_performance_pass=True)
        report["metadata"]["regression_gate"]["passed"] = False
        self.path.write_text(json.dumps(report))
        with self.assertRaisesRegex(RuntimeError, "does not match"):
            verify(self.path)


if __name__ == "__main__":
    unittest.main()
