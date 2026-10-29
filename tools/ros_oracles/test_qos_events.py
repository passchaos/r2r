import json
from pathlib import Path
import tempfile
import unittest
from unittest.mock import patch

from qos_events import verify


class QosReportBoundaries(unittest.TestCase):
    def test_endpoint_counter_and_source_identity_rejection(self):
        report = {"passed": True, "source_sha256": {"runtime.rs": "final"}, "records": {
            language: {"status": {"implementation": implementation, "offered_count": 1,
                "requested_count": 1, "policy": "reliability", "execution": "native_wait"}}
            for language, implementation in [("rust", "r2r"), ("cpp", "rclcpp")]
        }}
        with tempfile.TemporaryDirectory() as directory, patch("qos_events.source_identity", return_value={"runtime.rs": "final"}):
            path = Path(directory) / "report.json"
            path.write_text(json.dumps(report))
            verify(path, require_current_sources=True)
            for language in ["rust", "cpp"]:
                report["records"][language]["status"]["requested_count"] = 0
                path.write_text(json.dumps(report))
                with self.assertRaisesRegex(RuntimeError, "counts/policy"):
                    verify(path)
                report["records"][language]["status"]["requested_count"] = 1
            report["source_sha256"] = {"runtime.rs": "old"}
            path.write_text(json.dumps(report))
            with self.assertRaisesRegex(RuntimeError, "source drift"):
                verify(path, require_current_sources=True)
            del report["records"]["cpp"]
            path.write_text(json.dumps(report))
            with self.assertRaisesRegex(RuntimeError, "incomplete"):
                verify(path)


if __name__ == "__main__":
    unittest.main()
