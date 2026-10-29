import unittest

from action_feedback import SERVER_STAGES, matrix_pairs, regression_failures, validate_endpoint


class ActionFeedbackValidationTests(unittest.TestCase):
    def test_regression_matrix_contains_only_independent_rust_pairs(self):
        self.assertEqual(
            matrix_pairs(True, "regression"),
            [("baseline", "baseline"), ("rust", "rust")],
        )
        with self.assertRaisesRegex(RuntimeError, "independent baseline"):
            matrix_pairs(False, "regression")
        self.assertEqual(matrix_pairs(False, "server"), [("cpp", "cpp"), ("cpp", "rust")])

    def endpoint(self, role="ping"):
        result = {
            "implementation": "r2r", "role": role, "rmw": "rmw_test",
            "elements": 4, "goal_window": 2, "feedback_per_goal": 3,
            "iterations": 2, "warmup": 1, "received": 3,
            "batch_checksum": 3, "goal_checksum": 21,
            "accepted": 6, "feedback": 18, "results": 6,
        }
        if role == "ping":
            result.update(
                elapsed_ns=800, roundtrips_per_second=2 / 800e-9,
                feedback_messages_per_second=12 / 800e-9,
                rtt_ns_p50=400, rtt_ns_p95=400, rtt_ns_p99=400, rtt_ns_max=400,
                feedback_ns_p50=250, feedback_ns_p95=250, feedback_ns_p99=250,
                samples_ns=[300, 400], feedback_samples_ns=[200, 250])
        return result

    def test_complete_endpoint_oracle(self):
        validate_endpoint(self.endpoint(), "ping", "rmw_test", 4, 2, 3, 2, 1, "r2r", False)
        validate_endpoint(self.endpoint("echo"), "echo", "rmw_test", 4, 2, 3, 2, 1, "r2r", False)

    def test_server_stage_samples_and_percentiles_are_complete(self):
        result = self.endpoint("echo")
        result["server_stage_samples_ns"] = {stage: [10, 20] for stage in SERVER_STAGES}
        for percent in [50, 95, 99]:
            result[f"server_stage_ns_p{percent}"] = {stage: 20 for stage in SERVER_STAGES}
        validate_endpoint(
            result, "echo", "rmw_test", 4, 2, 3, 2, 1, "r2r", False, True
        )
        result["server_stage_samples_ns"]["terminal"].pop()
        with self.assertRaisesRegex(RuntimeError, "server stage samples"):
            validate_endpoint(result, "echo", "rmw_test", 4, 2, 3, 2, 1, server_stages=True)

    def test_declared_status_path_must_match_the_endpoint(self):
        result = self.endpoint()
        result["status_path"] = "batched"
        validate_endpoint(
            result, "ping", "rmw_test", 4, 2, 3, 2, 1, status_path="batched"
        )
        with self.assertRaisesRegex(RuntimeError, "status path mismatch"):
            validate_endpoint(
                result, "ping", "rmw_test", 4, 2, 3, 2, 1, status_path="per_goal"
            )

    def test_declared_status_sync_must_match_the_endpoint(self):
        result = self.endpoint()
        result["status_sync"] = "required"
        validate_endpoint(
            result, "ping", "rmw_test", 4, 2, 3, 2, 1, status_sync="required"
        )
        with self.assertRaisesRegex(RuntimeError, "status sync mismatch"):
            validate_endpoint(
                result, "ping", "rmw_test", 4, 2, 3, 2, 1, status_sync="none"
            )

    def test_report_qos_description_matches_structured_configuration(self):
        from pathlib import Path
        import gzip
        import hashlib
        import json
        import tempfile

        from verify_action_feedback import verify

        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            samples = gzip.compress(b"[]", mtime=0)
            (root / "samples.json.gz").write_bytes(samples)
            report = {
                "metadata": {
                    "workload": "action_feedback",
                    "payload_sizes": [4],
                    "goal_windows": [2],
                    "feedback_per_goal": 3,
                    "instrumentation": {"enabled": False, "latency_evidence": True},
                    "binary_sha256": {},
                    "matrix_scope": "full",
                    "status_qos": "default",
                    "result_timeout_ms": 1000,
                    "qos": ("goal/result/cancel/feedback reliable volatile "
                            "KeepLast(max(16, goal_window*feedback_per_goal)); "
                            "status native defaults; result retention 10 s"),
                    "samples_artifact": {
                        "file": "samples.json.gz",
                        "sha256": hashlib.sha256(samples).hexdigest(),
                        "record_count": 0,
                    },
                },
                "records": [],
                "summary": [],
            }
            path = root / "report.json"
            path.write_text(json.dumps(report))
            with self.assertRaisesRegex(RuntimeError, "QoS description mismatch"):
                verify(path)

    def test_payload_count_and_timing_corruption_fail(self):
        result = self.endpoint()
        result["feedback"] -= 1
        with self.assertRaisesRegex(RuntimeError, "count or checksum"):
            validate_endpoint(result, "ping", "rmw_test", 4, 2, 3, 2, 1)
        result = self.endpoint()
        result["feedback_samples_ns"][0] = result["samples_ns"][0] + 1
        with self.assertRaisesRegex(RuntimeError, "timing samples"):
            validate_endpoint(result, "ping", "rmw_test", 4, 2, 3, 2, 1)

    def test_regression_gate_covers_every_dimension_and_stage(self):
        summary = []
        for elements in [0, 4]:
            for window in [1, 2]:
                for language, multiplier in [("baseline", 1.0), ("rust", 1.04)]:
                    summary.append({
                        "elements": elements, "goal_window": window,
                        "ping_language": language, "echo_language": language,
                        **{field: 100 * multiplier for field in [
                            "rtt_ns_p50", "rtt_ns_p95", "rtt_ns_p99",
                            "feedback_ns_p50", "feedback_ns_p95", "feedback_ns_p99"]},
                        "feedback_messages_per_second": 100 / multiplier,
                    })
        self.assertEqual(regression_failures(summary, [0, 4], [1, 2], 1.05), [])
        summary[-1]["feedback_ns_p99"] = 106
        self.assertEqual(regression_failures(summary, [0, 4], [1, 2], 1.05)[0]["goal_window"], 2)


if __name__ == "__main__":
    unittest.main()
