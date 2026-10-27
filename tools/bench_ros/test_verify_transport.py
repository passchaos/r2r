import gzip
import hashlib
import json
from pathlib import Path
import tempfile
import unittest

from verify_transport import verify


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


if __name__ == "__main__":
    unittest.main()
