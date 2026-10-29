import unittest

from action_feedback import matrix_pairs, regression_failures, validate_endpoint


class ActionFeedbackValidationTests(unittest.TestCase):
    def test_regression_matrix_contains_only_independent_rust_pairs(self):
        self.assertEqual(
            matrix_pairs(True, "regression"),
            [("baseline", "baseline"), ("rust", "rust")],
        )
        with self.assertRaisesRegex(RuntimeError, "independent baseline"):
            matrix_pairs(False, "regression")

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
