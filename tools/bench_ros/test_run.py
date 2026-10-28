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

    def test_string_fixture_requires_an_explicit_matching_identity(self):
        validate_result(dict(self.result,fixture="string_array"), "rmw_cyclonedds_cpp",
                        "copy_reuse",32,1000,172,42,"string_array")
        for result,fixture in [(self.result,"string_array"),
                               (dict(self.result,fixture="string_array"),"int32_array"),
                               (dict(self.result,fixture="unknown"),"unknown")]:
            with self.subTest(fixture=fixture),self.assertRaisesRegex(RuntimeError,"fixture mismatch"):
                validate_result(result,"rmw_cyclonedds_cpp","copy_reuse",32,1000,172,42,fixture)

    def test_allocation_frames_require_matching_mode_and_valid_counters(self):
        frame={key:0 for key in ["malloc_calls","calloc_calls","realloc_calls","free_calls",
                                 "aligned_calls","failed_allocations","requested_bytes",
                                 "size_overflows","counter_overflows"]}
        result=dict(self.result,allocation_frame=frame)
        validate_result(result,"rmw_cyclonedds_cpp","copy_reuse",32,1000,172,42,probe_enabled=True)
        for record,enabled in [(result,False),(self.result,True)]:
            with self.assertRaisesRegex(RuntimeError,"probe mode mismatch"):
                validate_result(record,"rmw_cyclonedds_cpp","copy_reuse",32,1000,172,42,probe_enabled=enabled)
        for key,value in [("malloc_calls",-1),("realloc_calls",True),("requested_bytes",2**64),
                          ("failed_allocations",1),("size_overflows",1),("counter_overflows",1)]:
            corrupt=dict(result,allocation_frame=dict(frame,**{key:value}))
            with self.subTest(key=key),self.assertRaises(RuntimeError):
                validate_result(corrupt,"rmw_cyclonedds_cpp","copy_reuse",32,1000,172,42,probe_enabled=True)

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
