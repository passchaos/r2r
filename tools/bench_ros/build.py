#!/usr/bin/env python3
"""Build r2r and rclcpp comparison binaries, optionally including a Git baseline."""
import argparse
import hashlib
import json
import os
from pathlib import Path
import shutil
import subprocess
import tempfile

BENCHMARKS = {"codec": "ros_comparison", "transport": "pubsub_roundtrip", "native_views": "pubsub_view_roundtrip", "executor": "multi_node_service", "service": "service_roundtrip", "action": "action_roundtrip", "future": "future_execution"}
CPP_BINARIES = {"codec": "rclcpp_comparison", "transport": "rclcpp_roundtrip", "native_views": "rclcpp_roundtrip", "executor": "rclcpp_multi_node_service", "service": "rclcpp_service", "action": "rclcpp_action_roundtrip"}
CPP_SOURCES = {"codec": "comparison.cpp", "transport": "roundtrip.cpp", "native_views": "roundtrip.cpp", "executor": "multi_node_service.cpp", "service": "service.cpp", "action": "action.cpp"}


def command(arguments, cwd, env):
    result = subprocess.run(arguments, cwd=cwd, env=env, text=True, stdout=subprocess.PIPE, check=True)
    return result.stdout


def rust_binary(source, target, destination, env, workload="codec"):
    build_env = env.copy()
    build_env["CARGO_TARGET_DIR"] = str(target)
    benchmark = BENCHMARKS[workload]
    if workload == "executor":
        flags = build_env.get("RUSTFLAGS", "") + " --check-cfg=cfg(r2r_bench_shared_executor)"
        if (source / "r2r/src/nodes/executor.rs").exists():
            flags += " --cfg r2r_bench_shared_executor"
        build_env["RUSTFLAGS"] = flags.strip()
    if workload == "native_views":
        flags = build_env.get("RUSTFLAGS", "") + " --check-cfg=cfg(r2r_bench_native_views)"
        if (source / "r2r/src/native_views.rs").exists():
            flags += " --cfg r2r_bench_native_views"
        build_env["RUSTFLAGS"] = flags.strip()
    if workload == "codec":
        flags = build_env.get("RUSTFLAGS", "") + " --check-cfg=cfg(r2r_bench_serializer)"
        if (source / "r2r/src/serialization.rs").exists():
            flags += " --cfg r2r_bench_serializer"
        build_env["RUSTFLAGS"] = flags.strip()
    output = command(["cargo", "build", "--locked", "--release", "-p", "r2r", "--bench", benchmark, "--message-format=json"], source, build_env)
    artifacts = [json.loads(line) for line in output.splitlines() if line.startswith("{")]
    executable = next(item["executable"] for item in artifacts if item.get("reason") == "compiler-artifact" and item.get("target", {}).get("name") == benchmark and item.get("executable"))
    shutil.copy2(executable, destination)
    sources = [source / f"r2r/benches/{benchmark}.rs", source / "r2r/src/msg_types.rs", source / "r2r/src/nodes.rs", source / "r2r/src/subscribers.rs", source / "r2r/src/publishers.rs", source / "r2r/src/clients.rs", source / "r2r/src/services.rs", source / "r2r_rcl/src/lib.rs", source / "r2r_msg_gen/src/lib.rs", source / "Cargo.lock"]
    if (source / "r2r/src/serialization.rs").exists(): sources.append(source / "r2r/src/serialization.rs")
    if workload == "native_views":
        for relative in ["r2r/src/native_views.rs", "r2r/src/nodes/native_views.rs", "r2r_msg_gen/src/native_views.rs"]:
            if (source / relative).exists(): sources.append(source / relative)
    if workload == "executor":
        sources.append(source / "r2r/src/context.rs")
        sources.extend(sorted((source / "r2r/src/nodes").rglob("*.rs")))
        sources.append(source / "r2r/Cargo.toml")
    if workload == "action":
        sources.extend(source / f"r2r/src/{name}.rs" for name in ["action_common", "action_clients", "action_clients_untyped", "action_servers"])
        sources.extend(sorted((source / "r2r/src/action_clients").rglob("*.rs")))
        sources.extend(sorted((source / "r2r/benches/support").rglob("*.rs")))
    if workload == "future":
        sources.append(source / "r2r/src/context.rs")
        if (source / "r2r/src/nodes/future_executor.rs").exists():
            sources.append(source / "r2r/src/nodes/future_executor.rs")
    return {str(path.relative_to(source)): hashlib.sha256(path.read_bytes()).hexdigest() for path in sources}


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--baseline-ref", help="Git revision supplying the baseline library code")
    parser.add_argument("--workload", choices=list(BENCHMARKS), default="codec")
    parser.add_argument("--with-probe", action="store_true", help="build optional Linux/glibc allocation probe")
    parser.add_argument("--output-dir", type=Path, default=Path("target/ros-comparison"))
    args = parser.parse_args()
    if args.workload == "future" and args.with_probe:
        parser.error("future execution uses uninstrumented Rust baseline/candidate binaries")
    root = Path(__file__).resolve().parents[2]
    output = args.output_dir.resolve()
    output.mkdir(parents=True, exist_ok=True)
    env = os.environ.copy()
    manifests = {}
    manifests["workload"] = args.workload
    if args.workload == "executor":
        manifests["executor_api_detection"] = "nodes/executor.rs present: shared native wait; absent: Node native wait for one node and nonblocking round-robin for multiple nodes"
        manifests["benchmark_cfg"] = "--check-cfg=cfg(r2r_bench_shared_executor); --cfg r2r_bench_shared_executor only where the shared executor API exists"
    if args.workload == "native_views":
        manifests["receive_api_detection"] = "native_views.rs present: subscribe_view and publish_view echo; absent: normal typed receive/publish fallback"
        manifests["benchmark_cfg"] = "--check-cfg=cfg(r2r_bench_native_views); --cfg r2r_bench_native_views only where the view API exists"
    if args.workload == "codec":
        manifests["serializer_api_detection"] = "r2r/src/serialization.rs present: reusable API; absent: historical owned-byte API fallback for serialize_reuse modes"
        manifests["benchmark_cfg"] = "--check-cfg=cfg(r2r_bench_serializer); --cfg r2r_bench_serializer only when serializer API exists"
    benchmark = BENCHMARKS[args.workload]
    manifests["candidate"] = rust_binary(root, output / "candidate-cargo", output / "r2r-candidate", env, args.workload)
    if args.baseline_ref:
        revision = command(["git", "rev-parse", args.baseline_ref], root, env).strip()
        with tempfile.TemporaryDirectory(prefix="baseline-source-", dir=output) as directory:
            source = Path(directory)
            archive = output / "baseline-source.tar"
            command(["git", "archive", "--format=tar", f"--output={archive}", revision], root, env)
            command(["tar", "-xf", str(archive), "-C", str(source)], root, env)
            archive.unlink()
            shutil.copy2(root / f"r2r/benches/{benchmark}.rs", source / f"r2r/benches/{benchmark}.rs")
            if args.workload == "action":
                shutil.copytree(root / "r2r/benches/support", source / "r2r/benches/support", dirs_exist_ok=True)
            shutil.copy2(root / "Cargo.lock", source / "Cargo.lock")
            manifest = source / "r2r/Cargo.toml"
            if f'name = "{benchmark}"' not in manifest.read_text():
                manifest.write_text(manifest.read_text() + f'\n[[bench]]\nname = "{benchmark}"\nharness = false\n')
            if args.workload == "executor" and 'libc = ' not in manifest.read_text():
                manifest.write_text(manifest.read_text().replace('[dev-dependencies]\n', '[dev-dependencies]\nlibc = "0.2"\n', 1))
            manifests["baseline"] = rust_binary(source, output / "baseline-cargo", output / "r2r-baseline", env, args.workload)
        manifests["baseline_ref"] = revision
    if args.workload == "future":
        manifests["rustflags"] = env.get("RUSTFLAGS", "")
        manifests["cargo_release_opt_level"] = env.get("CARGO_PROFILE_RELEASE_OPT_LEVEL", "3")
        (output / "build-manifest.json").write_text(json.dumps(manifests,indent=2)+"\n")
        print(f"Future execution binaries and source manifest: {output}")
        return
    ros_prefix = f"/opt/ros/{env['ROS_DISTRO']}"
    command(["cmake", "-S", str(root / "tools/bench_ros"), "-B", str(output / "cpp"), "-DCMAKE_BUILD_TYPE=Release", f"-DCMAKE_PREFIX_PATH={ros_prefix}", "-DPython3_EXECUTABLE=/usr/bin/python3", f"-DR2R_BUILD_SERVICE_BENCH={'ON' if args.workload == 'service' else 'OFF'}", f"-DR2R_BUILD_ACTION_BENCH={'ON' if args.workload == 'action' else 'OFF'}", f"-DR2R_BUILD_EXECUTOR_BENCH={'ON' if args.workload == 'executor' else 'OFF'}", f"-DR2R_BUILD_ALLOCATION_PROBE={'ON' if args.with_probe else 'OFF'}"], root, env)
    cpp_binary = CPP_BINARIES[args.workload]
    command(["cmake", "--build", str(output / "cpp"), "--target", cpp_binary, "-j", "4"], root, env)
    shutil.copy2(output / f"cpp/{cpp_binary}", output / "rclcpp-comparison")
    manifests["cpp_source_sha256"] = {str(path.relative_to(root)): hashlib.sha256(path.read_bytes()).hexdigest() for path in [root / "tools/bench_ros/CMakeLists.txt", root / "tools/bench_ros" / CPP_SOURCES[args.workload]]}
    if args.workload == "action":
        manifests["cpp_source_sha256"].update({str(path.relative_to(root)): hashlib.sha256(path.read_bytes()).hexdigest() for path in sorted((root / "tools/bench_ros/action_support").rglob("*.hpp"))})
        native_header = root / "tools/bench_ros/probe/probe.h"
        manifests["cpp_source_sha256"][str(native_header.relative_to(root))] = hashlib.sha256(native_header.read_bytes()).hexdigest()
    if args.with_probe:
        command(["cmake", "--build", str(output / "cpp"), "--target", "r2r_bench_probe", "-j", "2"], root, env)
        shutil.copy2(output / "cpp/libr2r_bench_probe.so", output / "libr2r_bench_probe.so")
        manifests["probe_source_sha256"] = {str(path.relative_to(root)): hashlib.sha256(path.read_bytes()).hexdigest() for path in sorted((root / "tools/bench_ros/probe").glob("probe.*"))}
    manifests["rustflags"] = env.get("RUSTFLAGS", "")
    manifests["cargo_release_opt_level"] = env.get("CARGO_PROFILE_RELEASE_OPT_LEVEL", "3")
    (output / "build-manifest.json").write_text(json.dumps(manifests, indent=2) + "\n")
    print(f"Comparison binaries and source manifest: {output}")


if __name__ == "__main__":
    main()
