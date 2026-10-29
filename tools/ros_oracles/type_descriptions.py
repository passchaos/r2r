#!/usr/bin/env python3
"""Build and verify native Rust/rclcpp type-description service interoperability."""
import argparse
import hashlib
import json
import os
from pathlib import Path
import selectors
import shutil
import signal
import subprocess
import uuid
import re

ROOT = Path(__file__).resolve().parents[2]


def digest(path): return hashlib.sha256(path.read_bytes()).hexdigest()


def sources():
    paths = [ROOT / name for name in ["Cargo.lock", "Cargo.toml", "rust-toolchain.toml",
        "r2r/examples/type_description_interop.rs", "tools/ros_oracles/type_descriptions.py",
        "tools/ros_oracles/type_descriptions.cpp", "tools/ros_oracles/test_type_descriptions.py", "tools/ros_oracles/CMakeLists.txt"]]
    for crate in ["r2r", "r2r_common", "r2r_rcl", "r2r_msg_gen", "r2r_actions", "r2r_macros"]:
        for name in ["Cargo.toml", "build.rs"]:
            if (ROOT / crate / name).is_file(): paths.append(ROOT / crate / name)
        for directory in ["src", "bindings"]:
            paths.extend(path for path in (ROOT / crate / directory).rglob("*") if path.is_file())
    return {str(path.relative_to(ROOT)): digest(path) for path in sorted(set(paths))}


def build(output):
    output = output.resolve(); output.mkdir(parents=True, exist_ok=True)
    before = sources()
    with (output / "build.log").open("w") as log:
        subprocess.run(["cargo", "build", "--locked", "--release", "-p", "r2r", "--example", "type_description_interop"], cwd=ROOT, stdout=log, stderr=log, check=True)
        target = Path(os.environ.get("CARGO_TARGET_DIR", ROOT / "target"))
        if not target.is_absolute(): target = ROOT / target
        shutil.copy2(target / "release/examples/type_description_interop", output / "rust")
        subprocess.run(["cmake", "-S", str(ROOT / "tools/ros_oracles"), "-B", str(output / "cpp-build"), "-DCMAKE_BUILD_TYPE=Release", "-DPython3_EXECUTABLE=/usr/bin/python3"], stdout=log, stderr=log, check=True)
        subprocess.run(["cmake", "--build", str(output / "cpp-build"), "--target", "type_descriptions", "--parallel", "2"], stdout=log, stderr=log, check=True)
        shutil.copy2(output / "cpp-build/type_descriptions", output / "cpp")
    if before != sources(): raise RuntimeError("type-description inputs changed during build")
    (output / "build-manifest.json").write_text(json.dumps({"source_sha256": before,
        "binary_sha256": {name: digest(output / name) for name in ["rust", "cpp"]}}, indent=2) + "\n")
    print(f"{output}: independent type-description endpoints built")


def validate(records):
    if set(records) != {"rust_server", "cpp_server"}: raise RuntimeError("incomplete type-description language matrix")
    for key, record in records.items():
        ready, result = record["server"], record["client"]
        if not re.fullmatch(r"RIHS01_[0-9a-f]{64}", ready.get("hash", "")): raise RuntimeError("invalid native RIHS hash")
        if ready.get("kind") != "ready" or ready.get("role") != "server": raise RuntimeError("invalid schema server readiness")
        expected = {"kind": "result", "implementation": "rclcpp" if key == "rust_server" else "r2r",
                    "type_name": "std_msgs/msg/UInt64", "hash": ready["hash"], "fields": 1, "sources": True, "omission": True}
        core = {name: value for name, value in result.items() if name not in {"source_hex", "encoding", "exact_source"}}
        if core != expected: raise RuntimeError(f"type-description identity/source mismatch: {key}")
        if type(result.get("exact_source")) is not bool or result.get("encoding") not in {"msg", "idl"} or not re.fullmatch(r"(?:[0-9a-f]{2})+", result.get("source_hex", "")): raise RuntimeError("invalid source fidelity evidence")
    if records["rust_server"]["server"]["hash"] != records["cpp_server"]["server"]["hash"]:
        raise RuntimeError("native generated RIHS hash differs by language")
    if records["rust_server"]["client"]["source_hex"] != records["cpp_server"]["client"]["source_hex"]:
        raise RuntimeError("native source content differs by language")


def run(binaries, output, rmw, domain):
    manifest = json.loads((binaries / "build-manifest.json").read_text())
    if manifest["source_sha256"] != sources(): raise RuntimeError("type-description source drift")
    for name in ["rust", "cpp"]:
        if manifest["binary_sha256"][name] != digest(binaries / name): raise RuntimeError("type-description binary drift")
    env = os.environ.copy(); env.update(RMW_IMPLEMENTATION=rmw, ROS_DOMAIN_ID=str(domain), ROS_LOCALHOST_ONLY="1")
    for variable in ["CYCLONEDDS_URI", "FASTRTPS_DEFAULT_PROFILES_FILE", "FASTDDS_DEFAULT_PROFILES_FILE"]: env.pop(variable, None)
    records = {}; output.parent.mkdir(parents=True, exist_ok=True)
    for server_language, client_language in [("rust", "cpp"), ("cpp", "rust")]:
        name = "schema_" + uuid.uuid4().hex
        server = subprocess.Popen([str(binaries / server_language), "server", name], env=env, stdin=subprocess.PIPE, stdout=subprocess.PIPE, stderr=subprocess.PIPE, text=True)
        try:
            with selectors.DefaultSelector() as selector:
                selector.register(server.stdout, selectors.EVENT_READ)
                if not selector.select(15): raise RuntimeError("schema server readiness timeout")
                ready = json.loads(server.stdout.readline())
            client = subprocess.run([str(binaries / client_language), "client", name], env=env, text=True, capture_output=True, timeout=15)
            if client.returncode: raise RuntimeError(f"schema client failed: {client.stderr}")
            result = json.loads(client.stdout)
            if server_language == "cpp": server.send_signal(signal.SIGINT); _, stderr = server.communicate(timeout=5)
            else: _, stderr = server.communicate("finish\n", timeout=5)
            if server.returncode: raise RuntimeError(f"schema server failed: {stderr}")
            records[server_language + "_server"] = {"server": ready, "client": result, "server_stderr": stderr, "client_stderr": client.stderr}
        finally:
            if server.poll() is None: server.kill(); server.communicate()
    validate(records)
    if manifest["source_sha256"] != sources(): raise RuntimeError("type-description sources changed during run")
    installed = {key: (Path("/opt/ros") / env["ROS_DISTRO"] / "share/std_msgs/msg" / ("UInt64." + record["client"]["encoding"])).read_bytes().hex() for key, record in records.items()}
    source_pass = all(record["client"]["source_hex"] == installed[key] for key, record in records.items())
    report = {"scope": "independent Rust/rclcpp native schema/hash/source service directions",
              "ros_distro": env["ROS_DISTRO"], "rmw": rmw, "records": records, "installed_source_hex": installed,
              "exact_source_gate": {"passed": source_pass}, **manifest, "passed": True}
    output.write_text(json.dumps(report, indent=2) + "\n")
    print(f"{output}: both schema service language directions verified under {rmw}")


def verify(path, current, exact=False):
    report = json.loads(path.read_text())
    if report.get("passed") is not True: raise RuntimeError("type-description report did not pass")
    validate(report["records"])
    passed = all(record["client"]["source_hex"] == report["installed_source_hex"][key] for key, record in report["records"].items())
    if report["exact_source_gate"]["passed"] is not passed or any(record["client"]["exact_source"] is not (record["client"]["source_hex"] == report["installed_source_hex"][key]) for key, record in report["records"].items()): raise RuntimeError("source fidelity gate mismatch")
    if exact and not passed: raise RuntimeError("exact installed-source gate failed")
    if current and report["source_sha256"] != sources(): raise RuntimeError("type-description report source drift")
    print(f"{path}: schema service records verified")


def main():
    parser = argparse.ArgumentParser(description=__doc__); commands = parser.add_subparsers(dest="command", required=True)
    builder = commands.add_parser("build"); builder.add_argument("--output-dir", type=Path, required=True)
    runner = commands.add_parser("run"); runner.add_argument("--binaries", type=Path, required=True); runner.add_argument("--output", type=Path, required=True); runner.add_argument("--rmw", required=True); runner.add_argument("--domain-id", type=int, default=152)
    verifier = commands.add_parser("verify"); verifier.add_argument("reports", nargs="+", type=Path); verifier.add_argument("--require-current-sources", action="store_true")
    verifier.add_argument("--require-exact-sources", action="store_true")
    args = parser.parse_args()
    if args.command == "build": build(args.output_dir)
    elif args.command == "run": run(args.binaries.resolve(), args.output, args.rmw, args.domain_id)
    else:
        for path in args.reports: verify(path, args.require_current_sources, args.require_exact_sources)


if __name__ == "__main__": main()
