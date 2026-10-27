import copy
import unittest

from transport import validate_endpoint


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


if __name__ == "__main__":
    unittest.main()
