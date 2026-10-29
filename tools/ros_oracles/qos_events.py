#!/usr/bin/env python3
"""Verify independent Rust/rclcpp offered/requested QoS event notifications."""
import argparse
import hashlib
import json
import os
from pathlib import Path
import subprocess
import shutil
import uuid
import datetime
import xml.etree.ElementTree as ET
import selectors
import time

ROOT = Path(__file__).resolve().parents[2]


def source_identity():
    sources = [ROOT / relative for relative in ["Cargo.lock", "Cargo.toml", "rust-toolchain.toml",
        "r2r/examples/qos_event_diagnostics.rs", "tools/ros_oracles/qos_events.py",
        "tools/ros_oracles/qos_events.cpp", "tools/ros_oracles/test_qos_events.py", "tools/ros_oracles/CMakeLists.txt"]]
    for crate in ["r2r", "r2r_rcl", "r2r_common", "r2r_msg_gen", "r2r_actions", "r2r_macros"]:
        for name in ["Cargo.toml", "build.rs"]:
            if (ROOT / crate / name).is_file(): sources.append(ROOT / crate / name)
        sources.extend((ROOT / crate / "src").rglob("*.rs"))
        sources.extend((ROOT / crate / "bindings").rglob("*.rs"))
    return {str(path.relative_to(ROOT)): hashlib.sha256(path.read_bytes()).hexdigest() for path in sorted(set(sources))}


def build(output):
    output = output.resolve()
    output.mkdir(parents=True, exist_ok=True)
    before = source_identity()
    with (output / "build.log").open("w") as log:
        subprocess.run(["cargo", "build", "--locked", "--release", "-p", "r2r", "--example", "qos_event_diagnostics"],
                       cwd=ROOT, stdout=log, stderr=log, check=True)
        target = Path(os.environ.get("CARGO_TARGET_DIR", ROOT / "target"))
        if not target.is_absolute(): target = ROOT / target
        shutil.copy2(target / "release/examples/qos_event_diagnostics", output / "rust")
        subprocess.run(["cmake", "-S", str(ROOT / "tools/ros_oracles"), "-B", str(output / "cpp-build"),
                        "-DCMAKE_BUILD_TYPE=Release", "-DPython3_EXECUTABLE=/usr/bin/python3"], stdout=log, stderr=log, check=True)
        subprocess.run(["cmake", "--build", str(output / "cpp-build"), "--target", "qos_events", "--parallel", "2"],
                       stdout=log, stderr=log, check=True)
        shutil.copy2(output / "cpp-build/qos_events", output / "cpp")
    if source_identity() != before: raise RuntimeError("QoS source changed during endpoint builds")
    manifest = {"source_sha256": before, "binary_sha256": {
        name: hashlib.sha256((output / name).read_bytes()).hexdigest() for name in ["rust", "cpp"]}}
    (output / "build-manifest.json").write_text(json.dumps(manifest, indent=2) + "\n")
    print(f"{output}: native QoS endpoints and build manifest created")


def run(binaries, output, rmw, domain):
    rust, cpp = binaries / "rust", binaries / "cpp"
    manifest = json.loads((binaries / "build-manifest.json").read_text())
    if manifest["source_sha256"] != source_identity(): raise RuntimeError("QoS build manifest source drift")
    for name, path in [("rust", rust), ("cpp", cpp)]:
        if manifest["binary_sha256"][name] != hashlib.sha256(path.read_bytes()).hexdigest():
            raise RuntimeError(f"QoS endpoint binary drift: {name}")
    env = os.environ.copy()
    env.update(RMW_IMPLEMENTATION=rmw, ROS_DOMAIN_ID=str(domain), ROS_LOCALHOST_ONLY="1")
    for name in ["CYCLONEDDS_URI", "FASTRTPS_DEFAULT_PROFILES_FILE", "FASTDDS_DEFAULT_PROFILES_FILE"]:
        env.pop(name, None)
    prefix = "/qos_events_" + uuid.uuid4().hex
    processes = {}
    try:
        for language, command in [("rust", [str(rust), "--external", prefix]),
                                  ("cpp", [str(cpp), prefix])]:
            processes[language] = subprocess.Popen(command, env=env, stdin=subprocess.PIPE, stdout=subprocess.PIPE,
                                                    stderr=subprocess.PIPE, text=True)
        records = {}
        with selectors.DefaultSelector() as selector:
            buffers = {language: b"" for language in processes}
            for language, process in processes.items():
                os.set_blocking(process.stdout.fileno(), False)
                selector.register(process.stdout, selectors.EVENT_READ, language)
            deadline = time.monotonic() + 15
            while len(records) != len(processes):
                if time.monotonic() >= deadline: raise RuntimeError("native QoS endpoint readiness timed out")
                for key, _ in selector.select(0.5):
                    data = os.read(key.fileobj.fileno(), 65536)
                    if not data: raise RuntimeError(f"{key.data} exited before QoS readiness")
                    buffers[key.data] += data
                    if b"\n" in buffers[key.data]:
                        line, remainder = buffers[key.data].split(b"\n", 1)
                        if remainder: raise RuntimeError("unexpected extra QoS protocol output")
                        records[key.data] = {"status": json.loads(line)}
                        selector.unregister(key.fileobj)
        # Both endpoints retain their native entities until both status counters are observed.
        for language, process in processes.items():
            _, stderr = process.communicate("finish\n", timeout=5)
            if process.returncode:
                raise RuntimeError(f"{language} QoS endpoint failed ({process.returncode}): {stderr}")
            record = records[language]["status"]
            expected = {"implementation": "r2r" if language == "rust" else "rclcpp",
                        "offered_count": 1, "requested_count": 1,
                        "policy": "reliability", "execution": "native_wait"}
            if record != expected:
                raise RuntimeError(f"invalid {language} native event oracle: {record}")
            records[language]["stderr"] = stderr
    finally:
        for process in processes.values():
            if process.poll() is None:
                process.kill()
                process.communicate()
    if manifest["source_sha256"] != source_identity(): raise RuntimeError("QoS sources changed during oracle execution")
    prefix = Path("/opt/ros") / env["ROS_DISTRO"] / "share"
    report = {"scope": "independent-process native incompatible QoS counts and reliability policy",
              "verified_utc": datetime.datetime.now(datetime.timezone.utc).isoformat(),
              "git_head": subprocess.check_output(["git", "rev-parse", "HEAD"], cwd=ROOT, text=True).strip(),
              "rmw": rmw, "ros_distro": env["ROS_DISTRO"], "domain_id": domain,
              "ros_packages": {name: ET.parse(prefix / name / "package.xml").findtext("version") for name in ["rcl", "rclcpp", "rmw", rmw]},
              **manifest,
              "records": records, "passed": True}
    output.parent.mkdir(parents=True, exist_ok=True)
    output.write_text(json.dumps(report, indent=2) + "\n")
    print(f"{output}: Rust/rclcpp offered/requested counts and policy verified under {rmw}")


def verify(path, require_current_sources=False):
    report = json.loads(path.read_text())
    if report.get("passed") is not True or set(report.get("records", {})) != {"rust", "cpp"}:
        raise RuntimeError("incomplete native QoS endpoint report")
    for language in ["rust", "cpp"]:
        status = report["records"][language]["status"]
        if status != {"implementation": "r2r" if language == "rust" else "rclcpp",
                       "offered_count": 1, "requested_count": 1,
                       "policy": "reliability", "execution": "native_wait"}:
            raise RuntimeError(f"invalid native QoS counts/policy: {language}")
    if require_current_sources and report.get("source_sha256") != source_identity():
        raise RuntimeError("QoS report source drift")
    print(f"{path}: both endpoint status oracles verified")


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    commands = parser.add_subparsers(dest="command", required=True)
    builder = commands.add_parser("build")
    builder.add_argument("--output-dir", type=Path, required=True)
    runner = commands.add_parser("run")
    runner.add_argument("--binaries", type=Path, required=True)
    runner.add_argument("--output", type=Path, required=True)
    runner.add_argument("--rmw", required=True)
    runner.add_argument("--domain-id", type=int, default=138)
    verifier = commands.add_parser("verify")
    verifier.add_argument("reports", nargs="+", type=Path)
    verifier.add_argument("--require-current-sources", action="store_true")
    args = parser.parse_args()
    if args.command == "build": build(args.output_dir)
    elif args.command == "run": run(args.binaries.resolve(), args.output, args.rmw, args.domain_id)
    else:
        for path in args.reports: verify(path, args.require_current_sources)


if __name__ == "__main__":
    main()
