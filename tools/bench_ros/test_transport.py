import copy
import unittest

from transport import regression_failures, validate_endpoint


class EndpointValidation(unittest.TestCase):
    def setUp(self):
        self.echo = {"implementation": "r2r", "role": "echo", "rmw": "rmw_cyclonedds_cpp", "elements": 32,
                     "iterations": 3, "warmup": 1, "received": 4, "checksum": 6}
        self.ping = dict(self.echo, role="ping", samples_ns=[10, 20, 30], elapsed_ns=70,
                         roundtrips_per_second=3 / 70e-9, rtt_ns_p50=20,
                         rtt_ns_p95=30, rtt_ns_p99=30, rtt_ns_max=30)

    def validate(self, result):
        validate_endpoint(result, result["role"], "rmw_cyclonedds_cpp", 32, 3, 1, "r2r")

    def test_complete_oracle_passes(self):
        self.validate(self.echo)
        self.validate(self.ping)

    def test_wrong_peer_counts_payload_counts_and_rmw_fail(self):
        for field, value in [("implementation", "rclcpp"), ("received", 3), ("checksum", 5), ("rmw", "rmw_fastrtps_cpp"),
                             ("iterations", 1), ("warmup", 0), ("elements", 0)]:
            result = dict(self.echo)
            result[field] = value
            with self.subTest(field=field), self.assertRaises(RuntimeError):
                self.validate(result)

    def test_invalid_samples_and_percentiles_fail(self):
        for field, value in [("samples_ns", [10, 20]), ("samples_ns", [0, 20, 30]),
                             ("rtt_ns_p99", 20), ("elapsed_ns", 50),
                             ("roundtrips_per_second", float("nan")),
                             ("roundtrips_per_second", 1.0)]:
            result = copy.deepcopy(self.ping)
            result[field] = value
            with self.subTest(field=field), self.assertRaises(RuntimeError):
                self.validate(result)

    def test_regression_gate_catches_latency_and_throughput_losses(self):
        baseline = {"elements": 32, "ping_language": "baseline", "echo_language": "baseline",
                    "rtt_ns_p50": 100, "rtt_ns_p95": 100, "rtt_ns_p99": 100, "roundtrips_per_second": 100}
        candidate = dict(baseline, ping_language="rust", echo_language="rust")
        self.assertEqual(regression_failures([baseline, candidate], [32], 1.05), [])
        for field, value in [("rtt_ns_p50", 106), ("rtt_ns_p95", 106), ("rtt_ns_p99", 106),
                             ("roundtrips_per_second", 94)]:
            with self.subTest(field=field):
                worse = dict(candidate)
                worse[field] = value
                self.assertEqual(len(regression_failures([baseline, worse], [32], 1.05)), 1)

    def test_pipelined_service_latency_can_overlap_wall_time(self):
        result = dict(self.ping)
        result.pop("elements")
        result.update(window=16, elapsed_ns=40, roundtrips_per_second=3 / 40e-9)
        validate_endpoint(result, "ping", "rmw_cyclonedds_cpp", 16, 3, 1, "r2r", "window")
        result["elapsed_ns"] = 20
        result["roundtrips_per_second"] = 3 / 20e-9
        with self.assertRaisesRegex(RuntimeError, "throughput"):
            validate_endpoint(result, "ping", "rmw_cyclonedds_cpp", 16, 3, 1, "r2r", "window")

    def test_service_window_identity_and_latency_regressions_are_checked(self):
        result = dict(self.echo)
        result.pop("elements")
        result["window"] = 16
        with self.assertRaisesRegex(RuntimeError, "identity"):
            validate_endpoint(result, "echo", "rmw_cyclonedds_cpp", 1, 3, 1, "r2r", "window")
        baseline = {"window": 16, "ping_language": "baseline", "echo_language": "baseline",
                    "rtt_ns_p50": 100, "rtt_ns_p95": 100, "rtt_ns_p99": 100, "roundtrips_per_second": 100}
        candidate = dict(baseline, ping_language="rust", echo_language="rust", rtt_ns_p99=106)
        self.assertEqual(len(regression_failures([baseline, candidate], [16], 1.05, "window")), 1)

    def test_action_lifecycle_and_stage_oracles(self):
        result = dict(self.ping)
        result.pop("elements")
        result.update(scenario=1, accepted=4, feedback=4, results=4, canceled=4,
                      stage_samples_ns={"acceptance": [5, 10, 15], "cancellation": [2, 3, 4]},
                      acceptance_ns_p50=10, cancellation_ns_p50=3)
        validate_endpoint(result, "ping", "rmw_cyclonedds_cpp", 1, 3, 1, "r2r", "scenario")
        for field, value in [("accepted", 3), ("feedback", 3), ("results", 3), ("canceled", 0),
                             ("acceptance_ns_p50", 5), ("cancellation_ns_p50", None)]:
            bad = copy.deepcopy(result)
            bad[field] = value
            with self.subTest(field=field), self.assertRaises(RuntimeError):
                validate_endpoint(bad, "ping", "rmw_cyclonedds_cpp", 1, 3, 1, "r2r", "scenario")
        bad = copy.deepcopy(result)
        bad["stage_samples_ns"]["acceptance"] = [11, 10, 15]
        with self.assertRaisesRegex(RuntimeError, "exceeds"):
            validate_endpoint(bad, "ping", "rmw_cyclonedds_cpp", 1, 3, 1, "r2r", "scenario")
        success = dict(result, scenario=0, canceled=0, cancellation_ns_p50=None,
                       stage_samples_ns={"acceptance": [5, 10, 15], "cancellation": []})
        validate_endpoint(success, "ping", "rmw_cyclonedds_cpp", 0, 3, 1, "r2r", "scenario")


if __name__ == "__main__":
    unittest.main()
