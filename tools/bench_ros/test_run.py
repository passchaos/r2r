import unittest

from run import validate_result


class ResultValidation(unittest.TestCase):
    def setUp(self):
        self.result = {
            "rmw": "rmw_cyclonedds_cpp", "mode": "copy_reuse", "elements": 32,
            "iterations": 1000, "wire_bytes": 172, "wire_hash": 42,
            "elapsed_ns": 12_000, "ns_per_op": 12.0,
            "batch_ns_per_op_p50": 12.0, "batch_ns_per_op_p95": 12.0,
            "batch_ns_per_op_p99": 12.0,
        }

    def validate(self):
        validate_result(self.result, "rmw_cyclonedds_cpp", "copy_reuse", 32, 1000, 172, 42)

    def test_matching_result_is_accepted(self):
        self.validate()

    def test_wrong_workload_or_fixture_is_rejected(self):
        for field, value in [("rmw", "rmw_fastrtps_cpp"), ("iterations", 0), ("elements", 4096),
                             ("mode", "deserialize_owned"), ("wire_bytes", 0), ("wire_hash", 0)]:
            with self.subTest(field=field):
                saved = self.result[field]
                self.result[field] = value
                with self.assertRaises(RuntimeError):
                    self.validate()
                self.result[field] = saved

    def test_nonfinite_or_nonpositive_timings_are_rejected(self):
        for value in [float("nan"), float("inf"), 0, -1]:
            with self.subTest(value=value):
                self.result["ns_per_op"] = value
                with self.assertRaises(RuntimeError):
                    self.validate()

    def test_incorrect_operation_time_calculation_is_rejected(self):
        self.result["ns_per_op"] = 14.0
        with self.assertRaisesRegex(RuntimeError, "calculation"):
            self.validate()


if __name__ == "__main__":
    unittest.main()
