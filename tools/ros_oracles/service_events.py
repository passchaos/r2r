#!/usr/bin/env python3
"""Build, run and verify independent Rust/rclcpp service-introspection endpoints."""

import argparse
import hashlib
import json
import os
from pathlib import Path
import re
import selectors
import shutil
import subprocess
import time
import xml.etree.ElementTree as ET

ROOT = Path(__file__).resolve().parents[2]
MODES = ["off", "metadata", "contents", "off", "contents"]
REQUESTS = 3
CLOCK_NS = 12_000_000_034
SUPPORTED = {"iron", "jazzy", "kilted", "lyrical", "rolling"}
SOURCES = ["r2r/examples/service_introspection_interop.rs", "tools/ros_oracles/service_events.cpp",
           "tools/ros_oracles/service_events.py", "tools/ros_oracles/CMakeLists.txt"]


def digest(path):
    return hashlib.sha256(path.read_bytes()).hexdigest()


def source_identity():
    paths = [ROOT / path for path in SOURCES] + [ROOT / "Cargo.lock", ROOT / "Cargo.toml"]
    for package in ["r2r", "r2r_rcl", "r2r_common", "r2r_msg_gen", "r2r_actions", "r2r_macros"]:
        paths += [ROOT / package / "Cargo.toml", *sorted((ROOT / package / "src").rglob("*.rs"))]
    return {str(path.relative_to(ROOT)): digest(path) for path in paths}


def build(output):
    output = output.resolve()
    output.mkdir(parents=True, exist_ok=True)
    before = source_identity()
    env = os.environ.copy()
    env.setdefault("CARGO_BUILD_JOBS", "2")
    with (output / "build.log").open("w") as log:
        subprocess.run(["cargo", "build", "--locked", "--release", "-p", "r2r", "--example", "service_introspection_interop"],
                       cwd=ROOT, env=env, stdout=log, stderr=log, check=True)
        target = Path(env.get("CARGO_TARGET_DIR", str(ROOT / "target")))
        if not target.is_absolute():
            target = ROOT / target
        shutil.copy2(target / "release/examples/service_introspection_interop", output / "rust")
        subprocess.run(["cmake", "-S", str(ROOT / "tools/ros_oracles"), "-B", str(output / "cpp-build"),
                        "-DCMAKE_BUILD_TYPE=Release"], stdout=log, stderr=log, check=True)
        subprocess.run(["cmake", "--build", str(output / "cpp-build"), "--parallel", "2"], stdout=log, stderr=log, check=True)
        shutil.copy2(output / "cpp-build/service_events", output / "cpp")
    if source_identity() != before:
        raise RuntimeError("service-event source changed during build")
    manifest = {"source_sha256": before, "binary_sha256": {name: digest(output / name) for name in ["rust", "cpp"]},
                "ros_distro": os.environ["ROS_DISTRO"],
                "rustc": subprocess.check_output(["rustc", "--version"], text=True).strip(),
                "cxx": subprocess.check_output(["c++", "--version"], text=True).splitlines()[0]}
    (output / "build-manifest.json").write_text(json.dumps(manifest, indent=2) + "\n")
    print(f"Independent service-event endpoints built in {output}", flush=True)


class Pair:
    """Read both pipes without blocking on one endpoint or losing buffered lines."""

    def __init__(self, binaries, rust_role, representation, name, env, log_dir):
        self.selector = selectors.DefaultSelector()
        self.processes = {}
        self.logs = []
        self.buffers = {role: b"" for role in ["client", "server"]}
        self.messages = {role: [] for role in self.buffers}
        try:
            for role in self.buffers:
                language = "rust" if role == rust_role else "cpp"
                args = [str(binaries / language), role]
                if language == "rust":
                    args.append(representation)
                args.append(name)
                log = (log_dir / f"{role}.stderr.log").open("wb")
                self.logs.append(log)
                process = subprocess.Popen(args, env=env, stdin=subprocess.PIPE, stdout=subprocess.PIPE, stderr=log)
                self.processes[role] = process
                os.set_blocking(process.stdout.fileno(), False)
                self.selector.register(process.stdout, selectors.EVENT_READ, role)
        except BaseException:
            self.close()
            raise

    def receive(self, role, kind, phase=None):
        deadline = time.monotonic() + 20
        while True:
            if self.messages[role]:
                message = self.messages[role].pop(0)
                if message.get("kind") != kind or (phase is not None and message.get("phase") != phase):
                    raise RuntimeError(f"unexpected {role} message: {message}")
                return message
            remaining = deadline - time.monotonic()
            if remaining <= 0:
                raise RuntimeError(f"timed out waiting for {role} {kind}; inspect endpoint stderr logs")
            for key, _ in self.selector.select(min(remaining, 1)):
                data = os.read(key.fileobj.fileno(), 65536)
                source = key.data
                if not data:
                    self.selector.unregister(key.fileobj)
                    raise RuntimeError(f"{source} exited before the requested phase; inspect endpoint stderr logs")
                self.buffers[source] += data
                while b"\n" in self.buffers[source]:
                    line, self.buffers[source] = self.buffers[source].split(b"\n", 1)
                    self.messages[source].append(json.loads(line))

    def send(self, role, mode):
        pipe = self.processes[role].stdin
        pipe.write((mode + "\n").encode())
        pipe.flush()

    def finish(self):
        for process in self.processes.values():
            process.stdin.close()
        for role, process in self.processes.items():
            if process.wait(timeout=10) != 0:
                raise RuntimeError(f"{role} teardown failed")
            if self.messages[role] or self.buffers[role] or process.stdout.read():
                raise RuntimeError(f"unexpected trailing {role} output")

    def close(self):
        for process in self.processes.values():
            if process.poll() is None:
                process.terminate()
                try:
                    process.wait(timeout=5)
                except subprocess.TimeoutExpired:
                    process.kill()
                    process.wait()
            process.stdout.close()
            if not process.stdin.closed:
                process.stdin.close()
        self.selector.close()
        for log in self.logs:
            log.close()


def check(condition, message):
    if not condition:
        raise ValueError(message)


def verify_report(report):
    metadata = report["metadata"]
    check(metadata["ros_distro"] in SUPPORTED, "unsupported ROS distribution")
    check(metadata["modes"] == MODES and metadata["requests_per_phase"] == REQUESTS, "protocol dimensions")
    pairs = report["pairs"]
    expected_pairs = {(role, representation) for role in ["client", "server"] for representation in ["typed", "untyped"]}
    check(len(pairs) == 4 and {(p["rust_role"], p["representation"]) for p in pairs} == expected_pairs, "pair matrix")
    observed_events = 0
    for pair in pairs:
        check(len(pair["phases"]) == len(MODES), "phase matrix")
        all_sequences = set()
        native_identities = {role: set() for role in ["client", "server"]}
        for phase, (mode, record) in enumerate(zip(MODES, pair["phases"])):
            indexed = {}
            for role in ["client", "server"]:
                endpoint = record[role]
                expected_implementation = "r2r" if role == pair["rust_role"] else "rclcpp"
                expected_representation = pair["representation"] if role == pair["rust_role"] else "cpp"
                check(endpoint["kind"] == "phase" and endpoint["role"] == role, "endpoint role")
                check(endpoint["implementation"] == expected_implementation and endpoint["representation"] == expected_representation, "endpoint implementation")
                check(endpoint["phase"] == phase and endpoint["mode"] == mode and endpoint["service"] == pair["service"], "phase identity")
                check(endpoint["rmw"] == metadata["rmw"] and endpoint["clock_ns"] == CLOCK_NS, "native RMW/clock")
                check(endpoint["event_publishers"] == (0 if mode == "off" else 2), "event publisher graph")
                calls = endpoint["calls"]
                check(len(calls) == REQUESTS, "request count")
                indexed[role] = {}
                for i, call in enumerate(calls):
                    check(call["a"] == phase * REQUESTS + i + 1 and call["b"] == 23 and call["sum"] == call["a"] + 23, "request/response oracle")
                    sequence = call["sequence"]
                    check(type(sequence) is int and sequence > 0 and sequence not in indexed[role], "request sequence")
                    identity = call["client_gid" if role == "client" else "writer_guid"]
                    check(len(identity) == 16 and all(type(b) is int and 0 <= b <= 255 for b in identity) and any(identity), "native identity bytes")
                    native_identities[role].add(tuple(identity))
                    indexed[role][sequence] = call
            check(set(indexed["client"]) == set(indexed["server"]), "native request/response sequence mismatch")
            check(not all_sequences.intersection(indexed["client"]), "request sequence reused across phases")
            all_sequences.update(indexed["client"])
            for sequence, call in indexed["client"].items():
                check(all(call[field] == indexed["server"][sequence][field] for field in ["a", "b", "sum"]), "cross-endpoint payload mismatch")
            observations = []
            for role in ["client", "server"]:
                events = record[role]["events"]
                expected = set() if mode == "off" else {(sequence, kind) for sequence in indexed["client"] for kind in range(4)}
                check(len(events) == len(expected), "event count")
                observed = {}
                for event in events:
                    info = event["info"]
                    key = (info["sequence_number"], info["event_type"])
                    check(key in expected and key not in observed, "duplicate or unexpected event")
                    sequence, kind = key
                    check(info["stamp"] == {"sec": 12, "nanosec": 34}, "simulated event timestamp")
                    identity = indexed["client"][sequence]["client_gid"] if kind in [0, 3] else indexed["server"][sequence]["writer_guid"]
                    check(info["client_gid"] == identity, "event/native identity mismatch")
                    call = indexed["client"][sequence]
                    request = [{"a": call["a"], "b": call["b"]}] if mode == "contents" and kind in [0, 1] else []
                    response = [{"sum": call["sum"]}] if mode == "contents" and kind in [2, 3] else []
                    check(event["request"] == request and event["response"] == response, "event payload/state mismatch")
                    observed[key] = event
                check(set(observed) == expected, "missing event types")
                observations.append(observed)
                observed_events += len(events)
            check(observations[0] == observations[1], "Rust/C++ event observations differ")
        check(all(len(identities) == 1 for identities in native_identities.values()), "native endpoint identity changed during mode transitions")
    expected_summary = {"pairs": 4, "endpoint_phases": 40, "requests": 60, "observed_events": observed_events}
    check(report["summary"] == expected_summary and observed_events == 288, "summary mismatch")


def verify(path):
    report = json.loads(path.read_text())
    verify_report(report)
    print(f"{path}: 4 pairs, 40 endpoint phases, 60 requests and 288 event observations verified", flush=True)


def run(args):
    binaries = args.binaries.resolve()
    manifest = json.loads((binaries / "build-manifest.json").read_text())
    for language, expected in manifest["binary_sha256"].items():
        check(digest(binaries / language) == expected, f"{language} binary identity changed")
    output = args.output.resolve()
    output.parent.mkdir(parents=True, exist_ok=True)
    env = os.environ.copy()
    env.pop("CYCLONEDDS_URI", None)
    env.update(ROS_DOMAIN_ID=str(args.domain_id), RMW_IMPLEMENTATION=args.rmw, ROS_LOCALHOST_ONLY="1")
    metadata = {"ros_distro": os.environ["ROS_DISTRO"], "rmw": args.rmw, "domain_id": args.domain_id,
                "modes": MODES, "requests_per_phase": REQUESTS, "build_manifest": manifest, "package_versions": {}}
    check(manifest["ros_distro"] == metadata["ros_distro"], "build/runtime ROS distribution mismatch")
    prefixes = [Path(p) for p in env["AMENT_PREFIX_PATH"].split(os.pathsep) if p]
    for package in ["rclcpp", "rcl", "rmw", "example_interfaces", "service_msgs", args.rmw]:
        path = next((p / "share" / package / "package.xml" for p in prefixes if (p / "share" / package / "package.xml").exists()), None)
        check(path is not None, f"missing package metadata: {package}")
        metadata["package_versions"][package] = ET.parse(path).findtext("version")
    pairs = []
    for role in ["client", "server"]:
        for representation in ["typed", "untyped"]:
            name = f"/service_events_{os.getpid()}_{role}_{representation}"
            logs = output.parent / (output.stem + "-logs") / (role + "-" + representation)
            logs.mkdir(parents=True, exist_ok=True)
            pair = Pair(binaries, role, representation, name, env, logs)
            record = {"rust_role": role, "representation": representation, "service": name, "phases": []}
            pairs.append(record)
            try:
                for endpoint in ["server", "client"]:
                    pair.receive(endpoint, "started")
                for phase, mode in enumerate(MODES):
                    pair.send("server", mode)
                    pair.receive("server", "ready", phase)
                    pair.send("client", mode)
                    pair.receive("client", "ready", phase)
                    record["phases"].append({endpoint: pair.receive(endpoint, "phase", phase) for endpoint in ["server", "client"]})
                pair.finish()
            except BaseException:
                (output.parent / (output.stem + "-failed.json")).write_text(json.dumps({"metadata": metadata, "pairs": pairs}, indent=2) + "\n")
                raise
            finally:
                pair.close()
            print(f"{args.rmw}: Rust {representation} {role} passed five native mode phases", flush=True)
    report = {"metadata": metadata, "pairs": pairs,
              "summary": {"pairs": 4, "endpoint_phases": 40, "requests": 60, "observed_events": 288}}
    output.write_text(json.dumps(report, indent=2) + "\n")
    verify(output)


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    commands = parser.add_subparsers(dest="command", required=True)
    p = commands.add_parser("build")
    p.add_argument("--output-dir", type=Path, required=True)
    p = commands.add_parser("run")
    p.add_argument("--binaries", type=Path, required=True)
    p.add_argument("--output", type=Path, required=True)
    p.add_argument("--rmw", required=True)
    p.add_argument("--domain-id", type=int, default=142)
    p = commands.add_parser("verify")
    p.add_argument("reports", type=Path, nargs="+")
    args = parser.parse_args()
    if args.command != "verify" and os.environ.get("ROS_DISTRO") not in SUPPORTED:
        parser.error("service introspection requires Iron or newer; Humble is an unsupported capability boundary")
    if args.command == "build":
        build(args.output_dir)
    elif args.command == "run":
        if not 0 <= args.domain_id <= 232 or not re.fullmatch(r"rmw_[a-z0-9_]+", args.rmw):
            parser.error("invalid ROS domain/RMW identifier")
        run(args)
    else:
        for path in args.reports:
            verify(path)


if __name__ == "__main__":
    main()
