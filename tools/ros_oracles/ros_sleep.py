#!/usr/bin/env python3
"""Verify independent Rust/rclcpp ROS-clock sleep traces against one semantic oracle."""
import argparse
import hashlib
import json
import os
from pathlib import Path
import selectors
import shutil
import subprocess

ROOT = Path(__file__).resolve().parents[2]
COMMANDS = ["clock 100", "sleep 200", "clock 150", "pause", "clock 50", "clock 199", "clock 200", "sleep 300", "cancel", "finish"]
TIMES = [100, 100, 150, 150, 50, 199, 200, 200, 200, 200]
STATES = ["idle", "pending", "pending", "pending", "pending", "pending", "complete", "pending", "canceled", "canceled"]


def sources():
    paths = [ROOT / name for name in ["Cargo.toml", "Cargo.lock", "rust-toolchain.toml",
        "r2r/examples/ros_sleep_oracle.rs", "tools/ros_oracles/ros_sleep.py",
        "tools/ros_oracles/ros_sleep.cpp", "tools/ros_oracles/test_ros_sleep.py", "tools/ros_oracles/CMakeLists.txt"]]
    for crate in ["r2r", "r2r_rcl", "r2r_common", "r2r_msg_gen", "r2r_actions", "r2r_macros"]:
        for name in ["Cargo.toml", "build.rs"]:
            if (ROOT / crate / name).exists(): paths.append(ROOT / crate / name)
        for directory in ["src", "bindings"]:
            paths.extend(path for path in (ROOT / crate / directory).rglob("*") if path.is_file())
    return {str(path.relative_to(ROOT)): hashlib.sha256(path.read_bytes()).hexdigest() for path in sorted(set(paths))}


def build(output):
    output = output.resolve(); output.mkdir(parents=True, exist_ok=True)
    before = sources()
    with (output / "build.log").open("w") as log:
        subprocess.run(["cargo", "build", "--locked", "--release", "-p", "r2r", "--example", "ros_sleep_oracle"], cwd=ROOT, stdout=log, stderr=log, check=True)
        target = Path(os.environ.get("CARGO_TARGET_DIR", ROOT / "target"))
        if not target.is_absolute(): target = ROOT / target
        shutil.copy2(target / "release/examples/ros_sleep_oracle", output / "rust")
        subprocess.run(["cmake", "-S", str(ROOT / "tools/ros_oracles"), "-B", str(output / "cpp-build"), "-DCMAKE_BUILD_TYPE=Release", "-DPython3_EXECUTABLE=/usr/bin/python3"], stdout=log, stderr=log, check=True)
        subprocess.run(["cmake", "--build", str(output / "cpp-build"), "--target", "ros_sleep", "--parallel", "2"], stdout=log, stderr=log, check=True)
        shutil.copy2(output / "cpp-build/ros_sleep", output / "cpp")
    if before != sources(): raise RuntimeError("ROS sleep inputs changed during build")
    manifest = {"source_sha256": before, "binary_sha256": {name: hashlib.sha256((output / name).read_bytes()).hexdigest() for name in ["rust", "cpp"]}}
    (output / "build-manifest.json").write_text(json.dumps(manifest, indent=2) + "\n")
    print(f"{output}: ROS sleep native endpoints built")


def validate(records):
    if set(records) != {"rust", "cpp"}: raise RuntimeError("missing ROS sleep language trace")
    expected = [{"command":command, "now_ns":time, "state":state} for command, time, state in zip(COMMANDS, TIMES, STATES)]
    for language, record in records.items():
        if record["ready"] != {"kind":"ready", "implementation":"r2r" if language == "rust" else "rclcpp"}:
            raise RuntimeError(f"invalid ROS sleep endpoint identity: {language}")
        if record["trace"] != expected: raise RuntimeError(f"ROS time/deadline/cancellation trace mismatch: {language}")


def run(binaries, output, rmw, domain):
    manifest = json.loads((binaries / "build-manifest.json").read_text())
    if manifest["source_sha256"] != sources(): raise RuntimeError("ROS sleep source drift")
    records = {}
    for index, language in enumerate(["rust", "cpp"]):
        binary = binaries / language
        if manifest["binary_sha256"][language] != hashlib.sha256(binary.read_bytes()).hexdigest(): raise RuntimeError("ROS sleep binary drift")
        env = os.environ.copy(); env.update(RMW_IMPLEMENTATION=rmw, ROS_DOMAIN_ID=str(domain+index), ROS_LOCALHOST_ONLY="1")
        for name in ["CYCLONEDDS_URI", "FASTRTPS_DEFAULT_PROFILES_FILE", "FASTDDS_DEFAULT_PROFILES_FILE"]: env.pop(name, None)
        process = subprocess.Popen([str(binary)], env=env, stdin=subprocess.PIPE, stdout=subprocess.PIPE, stderr=subprocess.PIPE, text=True)
        try:
            with selectors.DefaultSelector() as selector:
                selector.register(process.stdout, selectors.EVENT_READ)
                def receive():
                    if not selector.select(15): raise RuntimeError(f"ROS sleep {language} trace timeout")
                    line = process.stdout.readline()
                    if not line: raise RuntimeError(f"ROS sleep {language} exited early: {process.stderr.read()}")
                    return json.loads(line)
                ready = receive(); trace = []
                for command in COMMANDS:
                    process.stdin.write(command + "\n"); process.stdin.flush(); trace.append(receive())
            _, stderr = process.communicate(timeout=5)
            if process.returncode: raise RuntimeError(f"ROS sleep endpoint failed: {stderr}")
            records[language] = {"ready":ready, "trace":trace, "stderr":stderr}
        finally:
            if process.poll() is None: process.kill(); process.communicate()
    validate(records)
    if manifest["source_sha256"] != sources(): raise RuntimeError("ROS sleep source changed during trace")
    output.parent.mkdir(parents=True, exist_ok=True)
    report = {"scope":"native ROS time sleep semantics; no performance claim", "ros_distro":os.environ["ROS_DISTRO"], "rmw":rmw, "records":records, **manifest, "passed":True}
    output.write_text(json.dumps(report, indent=2) + "\n")
    print(f"{output}: Rust/rclcpp pause/backward/absolute-target/cancellation traces verified")


def verify(path, current):
    report = json.loads(path.read_text())
    if report.get("passed") is not True: raise RuntimeError("ROS sleep report did not pass")
    validate(report["records"])
    if current and report["source_sha256"] != sources(): raise RuntimeError("ROS sleep report source drift")
    print(f"{path}: both native clock traces verified")


def main():
    parser = argparse.ArgumentParser(description=__doc__); commands = parser.add_subparsers(dest="command", required=True)
    builder = commands.add_parser("build"); builder.add_argument("--output-dir", type=Path, required=True)
    runner = commands.add_parser("run"); runner.add_argument("--binaries", type=Path, required=True); runner.add_argument("--output", type=Path, required=True); runner.add_argument("--rmw", required=True); runner.add_argument("--domain-id", type=int, default=158)
    verifier = commands.add_parser("verify"); verifier.add_argument("reports", nargs="+", type=Path); verifier.add_argument("--require-current-sources", action="store_true")
    args = parser.parse_args()
    if args.command == "build": build(args.output_dir)
    elif args.command == "run": run(args.binaries.resolve(), args.output, args.rmw, args.domain_id)
    else:
        for path in args.reports: verify(path, args.require_current_sources)


if __name__ == "__main__": main()
