#!/usr/bin/env python3
import gzip
import hashlib
import json
from pathlib import Path


HERE = Path(__file__).resolve().parent
ROOT = HERE.parents[2]


def digest(path):
    return hashlib.sha256(path.read_bytes()).hexdigest()


def require(relative, expected):
    path = HERE / relative
    if not path.is_file() or digest(path) != expected:
        raise RuntimeError(f"artifact digest mismatch: {relative}")
    return path


def log_text(entry):
    path = require(entry["artifact"], entry["artifact_sha256"])
    raw = gzip.decompress(path.read_bytes())
    if hashlib.sha256(raw).hexdigest() != entry["uncompressed_sha256"]:
        raise RuntimeError(f"uncompressed log digest mismatch: {entry['artifact']}")
    return raw.decode()


def main():
    results = json.loads((HERE / "results.json").read_text())
    if results["required_matrix"] != {
        "jazzy": ["rmw_cyclonedds_cpp", "rmw_fastrtps_cpp"],
        "humble": ["rmw_fastrtps_cpp"],
        "kilted": ["rmw_cyclonedds_cpp", "rmw_fastrtps_cpp"],
    }:
        raise RuntimeError("required ROS/RMW matrix mismatch")
    if not all(results["contract"].values()):
        raise RuntimeError("reset-event contract is incomplete")
    if results["no_ros_gate"] != {
        "public_documentation_artifacts": 84,
        "required_documented_methods": 341,
        "all_targets_compiled": True,
    }:
        raise RuntimeError("no-ROS inventory mismatch")

    jazzy = log_text(results["logs"]["jazzy"])
    compat = log_text(results["logs"]["compat"])
    no_ros = log_text(results["logs"]["no_ros"])
    full_release = log_text(results["logs"]["full_release"])
    for rmw in ("rmw_cyclonedds_cpp", "rmw_fastrtps_cpp"):
        if f"ROS_DISTRO=jazzy RMW_IMPLEMENTATION={rmw}" not in jazzy:
            raise RuntimeError(f"missing Jazzy {rmw} execution")
    if jazzy.count("test result: ok. 5 passed; 0 failed") != 4:
        raise RuntimeError("Jazzy debug/release reset-event count mismatch")
    if jazzy.count("test result: ok. 1 passed; 0 failed") != 4:
        raise RuntimeError("Jazzy callback/grouped test count mismatch")
    if jazzy.count("observed three resets and deterministic timer teardown") != 2:
        raise RuntimeError("Jazzy example count mismatch")
    if full_release.count("test result: ok. 822 passed; 0 failed; 20 ignored") != 2:
        raise RuntimeError("Jazzy full release suite count mismatch")

    expected_compat = (
        "ROS_DISTRO=humble RMW_IMPLEMENTATION=rmw_fastrtps_cpp",
        "ROS_DISTRO=kilted RMW_IMPLEMENTATION=rmw_cyclonedds_cpp",
        "ROS_DISTRO=kilted RMW_IMPLEMENTATION=rmw_fastrtps_cpp",
    )
    if any(marker not in compat for marker in expected_compat):
        raise RuntimeError("compatibility matrix marker missing")
    if compat.count("test result: ok. 5 passed; 0 failed") != 3:
        raise RuntimeError("compatibility reset-event count mismatch")
    if compat.count("test result: ok. 1 passed; 0 failed") != 3:
        raise RuntimeError("compatibility callback/grouped count mismatch")
    if compat.count("observed three resets and deterministic timer teardown") != 3:
        raise RuntimeError("compatibility example count mismatch")
    if "84 public documentation artifacts verified without ROS" not in no_ros:
        raise RuntimeError("no-ROS public documentation gate missing")

    for relative, expected in results["source_sha256"].items():
        path = ROOT / relative
        if not path.is_file() or digest(path) != expected:
            raise RuntimeError(f"source digest mismatch: {relative}")
    print("timer reset event validation artifacts verified")


if __name__ == "__main__":
    main()
