import gzip
import hashlib
import json
import os
from pathlib import Path
import tempfile
import subprocess
import sys
import unittest

from pubsub_pipeline import (
    matrix_pairs, rclcpp_failures, regression_failures, reserve_measurement_cpus, validate_endpoint,
    wait_for_isolation,
)
from verify_pubsub_pipeline import (
    current_source_mismatches, missing_pipeline_sources, verify, verify_compatibility_manifest,
    verify_isolation_gate,
)


class PipelineValidationTests(unittest.TestCase):
    @staticmethod
    def isolation_metadata(gate):
        return {
            "cpu_ping": 2,
            "cpu_echo": 3,
            "orchestrator_cpu_affinity": [0, 1],
            "isolation_gate": gate,
        }

    @staticmethod
    def isolation_gate():
        samples = [
            {"observed_utc": f"2026-10-29T00:00:0{index}+00:00", "load1": 0.5,
             "competing_processes": {}}
            for index in range(3)
        ]
        return {
            "passed": True,
            "preflight": {
                "required_consecutive_samples": 3, "interval_seconds": 20.0,
                "timeout_seconds": 300.0, "maximum_load1": 2.0,
                "forbidden_process_names": ["zig", "rustc"], "samples": samples,
                "passed": True,
            },
            "runtime": {
                "interval_seconds": 0.25, "maximum_allowed_load1": 2.0,
                "observation_count": 4, "maximum_load1": 1.9,
                "violations": [], "passed": True,
            },
        }

    def endpoint(self, role, language="rust"):
        common = {
            "implementation": "rclcpp" if language == "cpp" else "r2r",
            "role": role, "rmw": "rmw_test", "elements": 4, "window": 2, "fanout": 2,
            "iterations": 2, "warmup": 0, "received": 2, "fanout_received": 4,
            "checksum": 1, "fanout_checksum": 2, "max_inflight": 2,
        }
        if role == "ping":
            common.update(
                elapsed_ns=25, messages_per_second=80_000_000.0,
                rtt_ns_p50=20, rtt_ns_p95=20, rtt_ns_p99=20, rtt_ns_max=20,
                publish_ns_p50=3, publish_ns_p95=3, publish_ns_p99=3,
                samples_ns=[10, 20], publish_samples_ns=[2, 3])
        else:
            common.update(
                collect_ns_p50=5, collect_ns_p95=5, collect_ns_p99=5,
                publish_ns_p50=2, publish_ns_p95=2, publish_ns_p99=2,
                collect_samples_ns=[4, 5], publish_samples_ns=[1, 2])
        return common

    def test_endpoint_oracles_cover_window_counts_and_stages(self):
        validate_endpoint(self.endpoint("ping"), "ping", "rmw_test", 4, 2, 2, 2, 0)
        validate_endpoint(self.endpoint("echo"), "echo", "rmw_test", 4, 2, 2, 2, 0)
        path = self.endpoint("ping")
        path["receive_path"] = "native_callback"
        validate_endpoint(path, "ping", "rmw_test", 4, 2, 2, 2, 0,
                          receive_path="native_callback")
        with self.assertRaisesRegex(RuntimeError, "receive path mismatch"):
            validate_endpoint(path, "ping", "rmw_test", 4, 2, 2, 2, 0,
                              receive_path="stream")
        broken = self.endpoint("ping")
        broken["publish_samples_ns"][1] = 21
        with self.assertRaisesRegex(RuntimeError, "exceeds RTT"):
            validate_endpoint(broken, "ping", "rmw_test", 4, 2, 2, 2, 0)

    def test_matrix_and_gate_cover_complete_rtt_and_throughput(self):
        self.assertEqual(matrix_pairs(True, "regression"),
                         [("baseline", "baseline"), ("rust", "rust")])
        fields = {
            "rtt_ns_p50": 100, "rtt_ns_p95": 100, "rtt_ns_p99": 100,
            "publish_ns_p50": 10, "publish_ns_p95": 10, "publish_ns_p99": 10,
            "echo_collect_ns_p50": 30, "echo_collect_ns_p95": 30,
            "echo_collect_ns_p99": 30, "echo_publish_ns_p50": 10,
            "echo_publish_ns_p95": 10, "echo_publish_ns_p99": 10,
            "messages_per_second": 1000,
        }
        summary = [
            {"elements": 4, "window": 2, "fanout": 2,
             "ping_language": "baseline", "echo_language": "baseline", **fields},
            {"elements": 4, "window": 2, "fanout": 2,
             "ping_language": "rust", "echo_language": "rust", **fields},
        ]
        self.assertEqual(regression_failures(summary, [(4, 2, 2)], 1.05), [])
        summary[1]["rtt_ns_p99"] = 106
        self.assertEqual(regression_failures(summary, [(4, 2, 2)], 1.05)[0]["fanout"], 2)

    def test_runner_affinity_excludes_both_measurement_cpus(self):
        calls = []
        self.assertEqual(
            reserve_measurement_cpus(
                {0, 1, 2, 3}, 2, 3,
                set_affinity=lambda pid, cpus: calls.append((pid, cpus))),
            [0, 1],
        )
        self.assertEqual(calls, [(0, {0, 1})])
        with self.assertRaisesRegex(ValueError, "non-measurement CPU"):
            reserve_measurement_cpus({2, 3}, 2, 3, set_affinity=lambda *_: None)

    def test_runner_formal_ratio_gate_requires_three_idle_samples_before_starting(self):
        allowed = sorted(os.sched_getaffinity(0))
        if len(allowed) < 3:
            self.skipTest("pipeline runner requires three available CPUs")
        with tempfile.TemporaryDirectory() as directory:
            result = subprocess.run([
                sys.executable, str(Path(__file__).with_name("pubsub_pipeline.py")),
                "--rust", "unused-candidate", "--cpp", "unused-cpp",
                "--cpu-ping", str(allowed[0]), "--cpu-echo", str(allowed[1]),
                "--max-rclcpp-ratio", "1.0", "--isolation-samples", "2",
                "--output", str(Path(directory) / "unused.json"),
            ], text=True, capture_output=True)
            self.assertEqual(result.returncode, 2)
            self.assertIn("at least three idle isolation samples", result.stderr)

    def test_rclcpp_gate_requires_every_same_language_application_metric_to_lead(self):
        fields = {
            "rtt_ns_p50": 100, "rtt_ns_p95": 100, "rtt_ns_p99": 100,
            "messages_per_second": 1000,
        }
        summary = [
            {"elements": 4, "window": 2, "fanout": 2,
             "ping_language": "cpp", "echo_language": "cpp", **fields},
            {"elements": 4, "window": 2, "fanout": 2,
             "ping_language": "rust", "echo_language": "rust",
             **{**fields, "rtt_ns_p50": 99, "rtt_ns_p95": 99, "rtt_ns_p99": 100,
                "messages_per_second": 1001}},
        ]
        self.assertEqual(rclcpp_failures(summary, [(4, 2, 2)], 1.0), [])
        summary[1]["rtt_ns_p99"] = 101
        self.assertGreater(
            rclcpp_failures(summary, [(4, 2, 2)], 1.0)[0]["ratios"]["rtt_ns_p99"], 1.0)

    def test_isolation_wait_resets_until_three_consecutive_idle_samples(self):
        observations = iter([
            {"observed_utc": "0", "load1": 0.5, "competing_processes": {}},
            {"observed_utc": "1", "load1": 0.5, "competing_processes": {"zig": 1}},
            {"observed_utc": "2", "load1": 0.4, "competing_processes": {}},
            {"observed_utc": "3", "load1": 0.3, "competing_processes": {}},
            {"observed_utc": "4", "load1": 0.2, "competing_processes": {}},
        ])
        clock = [0.0]

        def sleep(seconds):
            clock[0] += seconds

        result = wait_for_isolation(
            3, 1.0, 10.0, 2.0, ("zig",), snapshot=lambda _: next(observations),
            sleeper=sleep, monotonic=lambda: clock[0])
        self.assertEqual([sample["observed_utc"] for sample in result["samples"]], ["2", "3", "4"])
        self.assertTrue(verify_isolation_gate(self.isolation_metadata(self.isolation_gate())))

    def test_archived_report_recomputes_raw_samples(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            records, traces, summary = [], [], []
            for language in ["baseline", "rust"]:
                ping, echo = self.endpoint("ping"), self.endpoint("echo")
                traces.append({
                    "record": len(records), "samples_ns": ping.pop("samples_ns"),
                    "ping_publish_samples_ns": ping.pop("publish_samples_ns"),
                    "echo_collect_samples_ns": echo.pop("collect_samples_ns"),
                    "echo_publish_samples_ns": echo.pop("publish_samples_ns"),
                })
                records.append({"elements": 4, "window": 2, "fanout": 2,
                                "ping_language": language, "echo_language": language,
                                "repetition": 0, "ping": ping, "echo": echo})
                summary.append({
                    "elements": 4, "window": 2, "fanout": 2,
                    "ping_language": language, "echo_language": language,
                    **{field: ping[field] for field in [
                        "rtt_ns_p50", "rtt_ns_p95", "rtt_ns_p99",
                        "publish_ns_p50", "publish_ns_p95", "publish_ns_p99",
                        "messages_per_second"]},
                    **{f"echo_{field}": echo[field] for field in [
                        "collect_ns_p50", "collect_ns_p95", "collect_ns_p99",
                        "publish_ns_p50", "publish_ns_p95", "publish_ns_p99"]},
                })
            compressed = gzip.compress(json.dumps(traces).encode(), mtime=0)
            (root / "samples.json.gz").write_bytes(compressed)
            report = {
                "metadata": {
                    "workload": "pubsub_pipeline", "rmw": "rmw_test",
                    "cpu_ping": 2, "cpu_echo": 3, "orchestrator_cpu_affinity": [0, 1],
                    "cases": [{"elements": 4, "window": 2, "fanout": 2}],
                    "iterations": 2, "warmup": 0, "repetitions": 1,
                    "matrix_scope": "regression", "binary_sha256": {"baseline": "a", "rust": "b"},
                    "samples_artifact": {"file": "samples.json.gz",
                                         "sha256": hashlib.sha256(compressed).hexdigest(),
                                         "record_count": 2},
                    "regression_gate": {"maximum_ratio": 1.05, "passed": True, "failures": []},
                    "isolation_gate": self.isolation_gate(),
                },
                "records": records, "summary": summary,
            }
            path = root / "report.json"
            path.write_text(json.dumps(report))
            verify(path, require_performance_pass=True)
            for limit in [float("nan"), float("inf"), 0.99]:
                report["metadata"]["regression_gate"]["maximum_ratio"] = limit
                path.write_text(json.dumps(report))
                with self.assertRaisesRegex(RuntimeError, "invalid pipeline regression ratio"):
                    verify(path, require_performance_pass=True)
            report["metadata"]["regression_gate"]["maximum_ratio"] = 1.05
            report["metadata"]["regression_gate"]["maximum_ratio"] = 1.06
            path.write_text(json.dumps(report))
            with self.assertRaisesRegex(RuntimeError, "passing pipeline regression gate"):
                verify(path, require_performance_pass=True)
            report["metadata"]["regression_gate"]["maximum_ratio"] = 1.05
            traces[0]["samples_ns"][0] = 0
            corrupted = gzip.compress(json.dumps(traces).encode(), mtime=0)
            (root / "samples.json.gz").write_bytes(corrupted)
            report["metadata"]["samples_artifact"]["sha256"] = hashlib.sha256(corrupted).hexdigest()
            path.write_text(json.dumps(report))
            with self.assertRaisesRegex(RuntimeError, "stage samples"):
                verify(path)

    def test_required_isolation_rejects_legacy_or_contended_reports(self):
        self.assertFalse(verify_isolation_gate({}))
        gate = self.isolation_gate()
        gate["runtime"]["violations"] = [{
            "observed_utc": "2026-10-29T00:00:05+00:00", "load1": 3.0,
            "competing_processes": {"zig": 1},
        }]
        gate["runtime"]["passed"] = False
        gate["passed"] = False
        self.assertFalse(verify_isolation_gate(self.isolation_metadata(gate)))

    def test_isolation_rejects_short_preflight_and_hidden_runtime_load_violation(self):
        gate = self.isolation_gate()
        gate["preflight"]["required_consecutive_samples"] = 1
        gate["preflight"]["samples"] = gate["preflight"]["samples"][:1]
        with self.assertRaisesRegex(RuntimeError, "invalid pipeline isolation preflight"):
            verify_isolation_gate(self.isolation_metadata(gate))
        gate = self.isolation_gate()
        gate["runtime"]["maximum_load1"] = 2.5
        # A summary with an excessive observed load cannot be qualified by removing violations.
        with self.assertRaisesRegex(RuntimeError, "invalid pipeline runtime isolation gate"):
            verify_isolation_gate(self.isolation_metadata(gate))
        gate = self.isolation_gate()
        gate["runtime"]["maximum_load1"] = 2.5
        gate["runtime"]["violations"] = [{
            "observed_utc": "2026-10-29T00:00:05+00:00", "load1": 2.5,
            "competing_processes": {},
        }]
        gate["runtime"]["passed"] = False
        gate["passed"] = False
        self.assertFalse(verify_isolation_gate(self.isolation_metadata(gate)))

    def test_current_source_manifest_detects_content_and_missing_file_drift(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            source = root / "src" / "candidate.rs"
            source.parent.mkdir()
            source.write_text("final\n")
            digest = hashlib.sha256(source.read_bytes()).hexdigest()
            metadata = {
                "source_sha256": {"src/candidate.rs": digest},
                "build_manifest": {
                    "candidate": {"src/candidate.rs": digest},
                    "cpp_source_sha256": {},
                },
            }
            self.assertEqual(current_source_mismatches(metadata, root), [])
            source.write_text("drift\n")
            self.assertEqual(
                current_source_mismatches(metadata, root)[0]["file"], "src/candidate.rs")
            source.unlink()
            self.assertIsNone(current_source_mismatches(metadata, root)[0]["observed"])

    def test_source_manifest_cannot_hide_an_extracted_runtime_module(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            source = root / "r2r/src/subscribers/native_callback.rs"
            source.parent.mkdir(parents=True)
            source.write_text("changed runtime implementation\n")
            metadata = {"source_sha256": {}}
            self.assertIn(str(source.relative_to(root)), missing_pipeline_sources(metadata, root))
            metadata["source_sha256"][str(source.relative_to(root))] = (
                hashlib.sha256(source.read_bytes()).hexdigest())
            self.assertNotIn(str(source.relative_to(root)), missing_pipeline_sources(metadata, root))

    def test_compatibility_manifest_binds_sources_and_distinct_logs(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            source = root / "candidate.rs"
            source.write_text("candidate\n")
            log = root / "validation.log.gz"
            log.write_bytes(gzip.compress(b"2 passed\n", mtime=0))
            manifest = {
                "source_sha256": {
                    "candidate.rs": hashlib.sha256(source.read_bytes()).hexdigest(),
                },
                "gates": {
                    "test": {
                        "log": log.name,
                        "sha256": hashlib.sha256(log.read_bytes()).hexdigest(),
                    },
                },
            }
            path = root / "compatibility.json"
            path.write_text(json.dumps(manifest))
            verify_compatibility_manifest(path, root)
            log.write_bytes(b"drift")
            with self.assertRaisesRegex(RuntimeError, "log digest mismatch"):
                verify_compatibility_manifest(path, root)


if __name__ == "__main__":
    unittest.main()
