#!/usr/bin/env python3
"""Record the isolation prerequisite before measuring parallel scaling."""
import json
from pathlib import Path
import sys

ROOT = Path(__file__).resolve().parents[3]
sys.path.insert(0, str(ROOT / "tools" / "bench_ros"))
from pubsub_pipeline import (  # noqa: E402
    DEFAULT_FORBIDDEN_PROCESS_NAMES,
    isolation_snapshot,
    wait_for_isolation,
)

observations = []
forbidden = (*DEFAULT_FORBIDDEN_PROCESS_NAMES, "lux-webgpu-comp")


def observe(names):
    sample = isolation_snapshot(names)
    observations.append(sample)
    print(json.dumps(sample), flush=True)
    return sample


settings = {
    "required_consecutive_samples": 3,
    "interval_seconds": 10,
    "timeout_seconds": 50,
    "maximum_load1": 2.0,
    "forbidden_process_names": list(forbidden),
}
try:
    result = wait_for_isolation(3, 10, 50, 2.0, forbidden, snapshot=observe)
except RuntimeError as error:
    result = {**settings, "passed": False, "error": str(error)}
result["observations"] = observations
result["performance_measurement_started"] = False
(ROOT / "target" / "callback-group-isolation-preflight.json").write_text(
    json.dumps(result, indent=2) + "\n"
)
sys.exit(0 if result["passed"] else 1)
