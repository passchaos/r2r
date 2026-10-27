import copy
import unittest

from transport import percentile, validate_endpoint
from action_support.validation import ALLOCATION_FIELDS, RESOURCE_FIELDS, language_pairs, validate_probe


def frame(implementation="r2r"):
    return {"abi": 1, "allocations": {name: 0 for name in ALLOCATION_FIELDS},
            "resources": {name: 0 for name in RESOURCE_FIELDS},
            "application": {"future_polls": 10, "spin_calls": 4,
                            "future_wakes": 7 if implementation == "r2r" else None}}


def endpoint():
    samples = [10, 20, 30, 40, 50, 60]
    result = {"implementation": "r2r", "role": "ping", "rmw": "rmw_cyclonedds_cpp", "scenario": 1,
              "iterations": 6, "warmup": 2, "received": 8, "checksum": 28,
              "accepted": 8, "feedback": 8, "results": 8, "canceled": 8,
              "samples_ns": samples, "elapsed_ns": 230, "cohort_process_elapsed_ns": 500, "roundtrips_per_second": 6 / 230e-9,
              "rtt_ns_max": 60, "stage_samples_ns": {"acceptance": [5, 10, 15, 20, 25, 30], "cancellation": [1, 2, 3, 4, 5, 6]},
              "acceptance_ns_p50": 20, "cancellation_ns_p50": 4,
              "cohort_config": {"count": 2, "iterations": 3, "warmup": 1, "maximum_retained_goals": 4, "fresh_endpoints": True},
              "cohorts": [{"index": 0, "sequence_offset": 0, "iterations": 3, "warmup": 1,
                           "received": 4, "accepted": 4, "feedback": 4, "results": 4, "canceled": 4, "checksum": 6, "elapsed_ns": 70},
                          {"index": 1, "sequence_offset": 4, "iterations": 3, "warmup": 1,
                           "received": 4, "accepted": 4, "feedback": 4, "results": 4, "canceled": 4, "checksum": 22, "elapsed_ns": 160}]}
    result.update({f"rtt_ns_p{percent}": percentile(samples, percent) for percent in [50, 95, 99]})
    return result


class ActionCohortValidation(unittest.TestCase):
    def test_focused_stability_scope_cannot_masquerade_as_full_interoperability(self):
        self.assertEqual(len(language_pairs(False)), 4)
        self.assertEqual(len(language_pairs(True)), 7)
        self.assertEqual(language_pairs(True, "action_stability"), [("baseline", "baseline"), ("rust", "rust")])
        with self.assertRaises(RuntimeError): language_pairs(False, "action_stability")
        with self.assertRaises(RuntimeError): language_pairs(True, "partial")

    def validate(self, result, instrumented=False):
        validate_endpoint(result, "ping", "rmw_cyclonedds_cpp", 1, 6, 2, "r2r", "scenario", 2, instrumented)

    def test_cohort_samples_are_combined_before_percentiles(self):
        result = endpoint()
        self.validate(result)
        result["rtt_ns_p50"] = 35  # Median of cohort p50s differs from combined percentile.
        with self.assertRaisesRegex(RuntimeError, "percentile"):
            self.validate(result)

    def test_fresh_endpoint_retention_and_sequence_boundaries_are_required(self):
        for key, value in [("count", 3), ("iterations", 2), ("warmup", 0),
                           ("maximum_retained_goals", 15), ("fresh_endpoints", False)]:
            result = endpoint(); result["cohort_config"][key] = value
            with self.subTest(key=key), self.assertRaises(RuntimeError):
                self.validate(result)
        for key, value in [("sequence_offset", 0), ("checksum", 6), ("received", 3),
                           ("feedback", 3), ("canceled", 0), ("elapsed_ns", 100), ("index", 0)]:
            result = endpoint(); result["cohorts"][1][key] = value
            with self.subTest(key=key), self.assertRaises(RuntimeError):
                self.validate(result)

    def test_cohort_time_excludes_setup_but_includes_every_measured_lifecycle(self):
        result = endpoint(); result["elapsed_ns"] += 1
        result["roundtrips_per_second"] = 6 / (result["elapsed_ns"] * 1e-9)
        with self.assertRaisesRegex(RuntimeError, "aggregate action elapsed"):
            self.validate(result)
        result = endpoint(); result["cohort_process_elapsed_ns"] = 200
        with self.assertRaisesRegex(RuntimeError, "process elapsed"):
            self.validate(result)

    def test_probe_frames_must_match_declared_mode_and_cover_every_cohort(self):
        result = endpoint()
        for cohort in result["cohorts"]: cohort["probe"] = frame()
        self.validate(result, True)
        with self.assertRaisesRegex(RuntimeError, "instrumentation"):
            self.validate(result, False)
        result["cohorts"][1].pop("probe")
        with self.assertRaisesRegex(RuntimeError, "missing action probe"):
            self.validate(result, True)

    def test_probe_rejects_overflow_impossible_failures_and_fabricated_cpp_wakes(self):
        validate_probe(frame(), "r2r")
        validate_probe(frame("rclcpp"), "rclcpp")
        for field, value in [("counter_overflows", 1), ("size_overflows", 1), ("malloc_calls", -1),
                             ("failed_allocations", 1), ("malloc_calls", True), ("malloc_calls", 2**64)]:
            value_frame = frame(); value_frame["allocations"][field] = value
            with self.subTest(field=field), self.assertRaises(RuntimeError):
                validate_probe(value_frame, "r2r")
        with self.assertRaisesRegex(RuntimeError, "unavailable"):
            validate_probe(frame(), "rclcpp")
        wrong = frame(); wrong["abi"] = 2
        with self.assertRaisesRegex(RuntimeError, "ABI"):
            validate_probe(wrong, "r2r")


if __name__ == "__main__":
    unittest.main()
