#!/usr/bin/env python3
"""Build r2r and rclcpp comparison binaries, optionally including a Git baseline."""
import argparse
import hashlib
import json
import os
import re
from pathlib import Path
import shutil
import subprocess
import tempfile

BENCHMARKS = {"codec": "ros_comparison", "codec_strings": "string_sequences", "transport": "pubsub_roundtrip", "pubsub_pipeline": "pubsub_pipeline", "native_views": "pubsub_view_roundtrip", "executor": "multi_node_service", "service": "service_roundtrip", "action": "action_roundtrip", "action_feedback": "action_feedback_roundtrip", "future": "future_execution", "callback_groups":"callback_groups"}
CPP_BINARIES = {"codec": "rclcpp_comparison", "codec_strings": "rclcpp_string_sequences", "transport": "rclcpp_roundtrip", "pubsub_pipeline": "rclcpp_pubsub_pipeline", "native_views": "rclcpp_roundtrip", "executor": "rclcpp_multi_node_service", "service": "rclcpp_service", "action": "rclcpp_action_roundtrip", "action_feedback": "rclcpp_action_feedback_roundtrip", "callback_groups":"rclcpp_callback_groups"}
CPP_SOURCES = {"codec": "comparison.cpp", "codec_strings": "string_sequences.cpp", "transport": "roundtrip.cpp", "pubsub_pipeline": "pubsub_pipeline.cpp", "native_views": "roundtrip.cpp", "executor": "multi_node_service.cpp", "service": "service.cpp", "action": "action.cpp", "action_feedback": "action_feedback.cpp", "callback_groups":"callback_groups.cpp"}


def command(arguments, cwd, env):
    result = subprocess.run(arguments, cwd=cwd, env=env, text=True, stdout=subprocess.PIPE, check=True)
    return result.stdout


def registry_lock_entries(path):
    entries=set()
    for block in re.findall(r"\[\[package\]\]\s*(.*?)(?=\n\[\[package\]\]|\Z)",path.read_text(),re.S):
        fields={key:json.loads(value) for key,value in re.findall(r'^(name|version|source)\s*=\s*("[^"\n]*")\s*$',block,re.M)}
        if "source" in fields:
            entries.add((fields["name"],fields["version"],fields["source"]))
    return entries


def pipeline_workspace_sources(source, benchmark="pubsub_pipeline"):
    paths = [source / name for name in ["Cargo.toml", "Cargo.lock", "rust-toolchain.toml"]]
    paths.append(source / f"r2r/benches/{benchmark}.rs")
    for crate in ["r2r", "r2r_common", "r2r_rcl", "r2r_msg_gen", "r2r_actions", "r2r_macros"]:
        for name in ["Cargo.toml", "build.rs"]:
            if (source / crate / name).exists():
                paths.append(source / crate / name)
        paths.extend(sorted((source / crate / "src").rglob("*.rs")))
        paths.extend(sorted((source / crate / "build").rglob("*.rs")))
        paths.extend(sorted((source / crate / "bindings").rglob("*.rs")))
    return sorted(set(paths))


def snapshot_inputs(root,paths):
    return {str(path.relative_to(root)):hashlib.sha256(path.read_bytes()).hexdigest() for path in sorted(set(paths))}

def require_unchanged_inputs(root,snapshot):
    changed=[name for name,sha in snapshot.items() if not (root/name).is_file() or hashlib.sha256((root/name).read_bytes()).hexdigest()!=sha]
    if changed:raise RuntimeError("benchmark inputs changed during compilation: "+", ".join(changed))

def build_jobs(env,default=4):
    raw=env.get("R2R_BUILD_JOBS",env.get("CARGO_BUILD_JOBS",str(default)))
    try:jobs=int(raw)
    except ValueError as error:raise RuntimeError(f"invalid build parallelism: {raw}") from error
    if jobs<1:raise RuntimeError(f"invalid build parallelism: {raw}")
    return jobs

def pubsub_receive_configuration(source, requested):
    if requested not in {"stream", "native_callback", "reusable_typed", "batch_typed"}:
        raise RuntimeError(f"invalid pubsub receive path: {requested}")
    callback_source = source / "r2r/src/nodes/subscription_api.rs"
    callback_text = callback_source.read_text() if callback_source.exists() else ""
    if requested == "native_callback" and "pub fn subscribe_native_callback" in callback_text:
        return "native_callback", " --cfg r2r_bench_native_callbacks"
    if requested == "reusable_typed" and "pub fn subscribe_reusable" in callback_text:
        return "reusable_typed", " --cfg r2r_bench_reusable_typed"
    if requested == "batch_typed" and "pub fn subscribe_batches" in callback_text:
        return "batch_typed", " --cfg r2r_bench_batch_typed"
    return "stream", ""

def rust_binary(source, target, destination, env, workload="codec"):
    build_env = env.copy()
    build_env["CARGO_TARGET_DIR"] = str(target)
    benchmark = BENCHMARKS[workload]
    if workload == "executor":
        flags = build_env.get("RUSTFLAGS", "") + " --check-cfg=cfg(r2r_bench_shared_executor)"
        if (source / "r2r/src/nodes/executor.rs").exists():
            flags += " --cfg r2r_bench_shared_executor"
        flags += " --check-cfg=cfg(r2r_bench_executor_control)"
        if (source / "r2r/src/nodes/executor/control.rs").exists():
            flags += " --cfg r2r_bench_executor_control"
        flags += " --check-cfg=cfg(r2r_bench_background_task)"
        if (source / "r2r/src/nodes/executor/background.rs").exists():
            flags += " --cfg r2r_bench_background_task"
        build_env["RUSTFLAGS"] = flags.strip()
    if workload == "native_views":
        flags = build_env.get("RUSTFLAGS", "") + " --check-cfg=cfg(r2r_bench_native_views)"
        if (source / "r2r/src/native_views.rs").exists():
            flags += " --cfg r2r_bench_native_views"
        build_env["RUSTFLAGS"] = flags.strip()
    if workload == "pubsub_pipeline":
        receive_path = build_env.get("R2R_BENCH_PUBSUB_RECEIVE_PATH", "native_callback")
        _, receive_cfg = pubsub_receive_configuration(source, receive_path)
        flags = (build_env.get("RUSTFLAGS", "")
                 + " --check-cfg=cfg(r2r_bench_native_callbacks)"
                 + " --check-cfg=cfg(r2r_bench_reusable_typed)"
                 + " --check-cfg=cfg(r2r_bench_batch_typed)")
        flags += receive_cfg
        build_env["RUSTFLAGS"] = flags.strip()
    if workload in {"codec", "codec_strings"}:
        flags = build_env.get("RUSTFLAGS", "") + " --check-cfg=cfg(r2r_bench_serializer)"
        if (source / "r2r/src/serialization.rs").exists():
            flags += " --cfg r2r_bench_serializer"
        build_env["RUSTFLAGS"] = flags.strip()
    if workload == "action_feedback":
        flags = (build_env.get("RUSTFLAGS", "")
                 + " --check-cfg=cfg(r2r_bench_action_batches)"
                 + " --check-cfg=cfg(r2r_bench_native_action_feedback)")
        batch_source = source / "r2r/src/action_servers/batch.rs"
        if batch_source.exists() and "pub fn execute_batch" in batch_source.read_text():
            flags += " --cfg r2r_bench_action_batches"
        client_source = source / "r2r/src/action_clients.rs"
        if (client_source.exists()
                and "pub fn send_goal_request_native_feedback" in client_source.read_text()):
            flags += " --cfg r2r_bench_native_action_feedback"
        build_env["RUSTFLAGS"] = flags.strip()
    if workload == "service":
        flags = build_env.get("RUSTFLAGS", "") + " --check-cfg=cfg(r2r_bench_service_callbacks)"
        callback_source = source / "r2r/src/nodes/service_callbacks.rs"
        if callback_source.exists() and "pub fn create_service_native_callback" in callback_source.read_text():
            flags += " --cfg r2r_bench_service_callbacks"
        build_env["RUSTFLAGS"] = flags.strip()
    callback_snapshot=None
    if workload == "callback_groups":
        callback_paths=[*pipeline_workspace_sources(source,benchmark),*sorted((source/"r2r/benches/support/callback_groups").rglob("*.rs")),source/"r2r/benches/support/allocation_probe.rs"]
        callback_snapshot=snapshot_inputs(source,callback_paths)
    output = command(["cargo", "build", "--locked", "--release", "-p", "r2r", "--bench", benchmark, "--message-format=json"], source, build_env)
    artifacts = [json.loads(line) for line in output.splitlines() if line.startswith("{")]
    executable = next(item["executable"] for item in artifacts if item.get("reason") == "compiler-artifact" and item.get("target", {}).get("name") == benchmark and item.get("executable"))
    if callback_snapshot is not None:require_unchanged_inputs(source,callback_snapshot)
    shutil.copy2(executable, destination)
    sources = [source / f"r2r/benches/{benchmark}.rs", source / "r2r/Cargo.toml", source / "r2r/src/msg_types.rs", source / "r2r/src/nodes.rs", source / "r2r/src/subscribers.rs", source / "r2r/src/publishers.rs", source / "r2r/src/clients.rs", source / "r2r/src/services.rs", source / "r2r_rcl/src/lib.rs", source / "r2r_msg_gen/src/lib.rs", source / "Cargo.lock"]
    sources.extend(source / f"r2r/src/{name}.rs" for name in ["lib", "error", "availability"])
    if (source / "r2r/src/native_metadata.rs").exists():
        sources.append(source / "r2r/src/native_metadata.rs")
    if (source / "r2r/src/notifications.rs").exists():
        sources.append(source / "r2r/src/notifications.rs")
    if workload in {"service", "executor", "pubsub_pipeline"}:
        sources.extend(sorted((source / "r2r/src/nodes").rglob("*.rs")))
        sources.extend(sorted((source / "r2r/src/lifecycle").rglob("*.rs")))
        if (source / "r2r/src/lifecycle.rs").exists():
            sources.append(source / "r2r/src/lifecycle.rs")
    if workload == "pubsub_pipeline":
        # Native receive behavior depends on generated bindings, build scripts and shared
        # runtime modules too. Cover the complete workspace input set so an extraction or
        # generator change cannot leave a stale performance report apparently current.
        sources.extend(pipeline_workspace_sources(source))
    if workload in {"service","callback_groups"}:
        sources.extend(pipeline_workspace_sources(source, benchmark))
    if workload == "callback_groups":
        sources.extend(sorted((source/"r2r/benches/support/callback_groups").rglob("*.rs")))
        sources.append(source/"r2r/benches/support/allocation_probe.rs")
    if workload in {"codec", "codec_strings"}:
        sources.append(source / "r2r/benches/support/codec.rs")
        sources.append(source / "r2r/benches/support/allocation_probe.rs")
        for relative in ["r2r/src/lib.rs","r2r/src/error.rs","r2r/src/parameters.rs","r2r/src/strings.rs","r2r_msg_gen/src/introspection.rs"]:
            if (source/relative).exists(): sources.append(source/relative)
        sources.extend(sorted((source/"r2r/src/msg_types").rglob("*.rs")))
        sources.extend(sorted((source/"r2r/src/strings").rglob("*.rs")))
        sources.extend(sorted((source / "r2r_rcl/src/strings").rglob("*.rs")))
        if (source / "r2r_rcl/src/strings.rs").exists(): sources.append(source / "r2r_rcl/src/strings.rs")
    if (source / "r2r/src/serialization.rs").exists(): sources.append(source / "r2r/src/serialization.rs")
    if workload == "native_views":
        for relative in ["r2r/src/native_views.rs", "r2r/src/nodes/native_views.rs", "r2r_msg_gen/src/native_views.rs"]:
            if (source / relative).exists(): sources.append(source / relative)
    if workload == "executor":
        sources.append(source / "r2r/src/context.rs")
        sources.extend(sorted((source / "r2r/src/nodes").rglob("*.rs")))
    if workload in {"action", "action_feedback"}:
        sources.extend(source / f"r2r/src/{name}.rs" for name in ["action_common", "action_clients", "action_clients_untyped", "action_servers"])
        sources.extend(sorted((source / "r2r/src/action_clients").rglob("*.rs")))
        sources.extend(sorted((source / "r2r/src/action_servers").rglob("*.rs")))
        sources.extend(sorted((source / "r2r/src/nodes").rglob("*.rs")))
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
        manifests["executor_control_detection"] = "nodes/executor/control.rs present: cancellation handle available; R2R_BENCH_CONTROL_ENABLED creates and retains a handle before measurement"
        manifests["background_api_detection"] = "nodes/executor/background.rs present: Executor::spawn task; absent: manual native wait thread and join; R2R_BENCH_BACKGROUND_ENABLED selects runtime-independent block_on application driver"
        manifests["benchmark_cfg"] = "--check-cfg declares shared_executor, executor_control and background_task; --cfg enables each only where its API exists"
    if args.workload == "native_views":
        manifests["receive_api_detection"] = "native_views.rs present: subscribe_view and publish_view echo; absent: normal typed receive/publish fallback"
        manifests["benchmark_cfg"] = "--check-cfg=cfg(r2r_bench_native_views); --cfg r2r_bench_native_views only where the view API exists"
    if args.workload == "pubsub_pipeline":
        manifests["native_callback_api_detection"] = "nodes/subscription_api.rs contains public subscribe_native_callback: scoped native callback receive/forward; absent: owned typed stream fallback"
        manifests["reusable_typed_api_detection"] = "nodes/subscription_api.rs contains public subscribe_reusable: capacity-reusing typed stream; absent: owned typed stream fallback"
        manifests["batch_typed_api_detection"] = "nodes/subscription_api.rs contains public subscribe_batches: rcl_take_sequence typed batch stream; absent: owned typed stream fallback"
        manifests["requested_receive_path"] = env.get("R2R_BENCH_PUBSUB_RECEIVE_PATH", "native_callback")
        manifests["benchmark_cfg"] = "declares native_callbacks, reusable_typed and batch_typed; enables only the explicitly requested public API, otherwise stream fallback"
    if args.workload == "service":
        manifests["service_callback_api_detection"] = "nodes/service_callbacks.rs contains create_service_native_callback: native request/reused response; absent: owned service stream"
        manifests["benchmark_cfg"] = "--check-cfg=cfg(r2r_bench_service_callbacks); --cfg enables native service callbacks only where their API exists"
    if args.workload in {"codec", "codec_strings"}:
        manifests["serializer_api_detection"] = "r2r/src/serialization.rs present: reusable API; absent: historical owned-byte API fallback for serialize_reuse modes"
        manifests["benchmark_cfg"] = "--check-cfg=cfg(r2r_bench_serializer); --cfg r2r_bench_serializer only when serializer API exists"
    if args.workload == "action_feedback":
        manifests["batch_api_detection"] = "action_servers/batch.rs contains public execute_batch: use batched accept/execute/terminal status snapshots; absent: identical harness falls back to per-goal operations"
        manifests["native_feedback_api_detection"] = "action_clients.rs contains public send_goal_request_native_feedback: selectable native delivered feedback; absent: owned delivery only"
        manifests["benchmark_cfg"] = "--check-cfg declares action_batches and native_action_feedback; --cfg enables each only where its public API exists"
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
            if args.workload in {"codec", "codec_strings"}:
                (source / "r2r/benches/support").mkdir(parents=True, exist_ok=True)
                shutil.copy2(root / "r2r/benches/support/codec.rs", source / "r2r/benches/support/codec.rs")
                shutil.copy2(root / "r2r/benches/support/allocation_probe.rs", source / "r2r/benches/support/allocation_probe.rs")
            if args.workload in {"action", "action_feedback", "callback_groups"}:
                shutil.copytree(root / "r2r/benches/support", source / "r2r/benches/support", dirs_exist_ok=True)
            shutil.copy2(root / "Cargo.lock", source / "Cargo.lock")
            manifest = source / "r2r/Cargo.toml"
            if f'name = "{benchmark}"' not in manifest.read_text():
                manifest.write_text(manifest.read_text() + f'\n[[bench]]\nname = "{benchmark}"\nharness = false\n')
            if args.workload == "executor" and 'libc = ' not in manifest.read_text():
                manifest.write_text(manifest.read_text().replace('[dev-dependencies]\n', '[dev-dependencies]\nlibc = "0.2"\n', 1))
            # Workspace dependency declarations may change while registry versions stay pinned.
            # Reconcile that metadata offline, then reject any new resolved dependency identity.
            command(["cargo","metadata","--offline","--format-version=1"],source,env)
            if not registry_lock_entries(source/"Cargo.lock").issubset(registry_lock_entries(root/"Cargo.lock")):
                raise RuntimeError("baseline dependency versions differ from the candidate lock")
            manifests["baseline"] = rust_binary(source, output / "baseline-cargo", output / "r2r-baseline", env, args.workload)
        manifests["baseline_ref"] = revision
    if args.workload == "future":
        manifests["rustflags"] = env.get("RUSTFLAGS", "")
        manifests["cargo_release_opt_level"] = env.get("CARGO_PROFILE_RELEASE_OPT_LEVEL", "3")
        (output / "build-manifest.json").write_text(json.dumps(manifests,indent=2)+"\n")
        print(f"Future execution binaries and source manifest: {output}")
        return
    callback_cpp_snapshot=None
    if args.workload == "callback_groups":
        callback_paths=[root/"tools/bench_ros/CMakeLists.txt",root/"tools/bench_ros/callback_groups.cpp",*sorted((root/"tools/bench_ros/callback_group_support").rglob("*.hpp")),root/"tools/bench_ros/codec_support/probe.hpp",root/"tools/bench_ros/probe/probe.h"]
        callback_cpp_snapshot=snapshot_inputs(root,callback_paths)
    ros_prefix = f"/opt/ros/{env['ROS_DISTRO']}"
    command(["cmake", "-S", str(root / "tools/bench_ros"), "-B", str(output / "cpp"), "-DCMAKE_BUILD_TYPE=Release", f"-DCMAKE_PREFIX_PATH={ros_prefix}", "-DPython3_EXECUTABLE=/usr/bin/python3", f"-DR2R_BUILD_SERVICE_BENCH={'ON' if args.workload == 'service' else 'OFF'}", f"-DR2R_BUILD_PUBSUB_PIPELINE_BENCH={'ON' if args.workload == 'pubsub_pipeline' else 'OFF'}", f"-DR2R_BUILD_ACTION_BENCH={'ON' if args.workload == 'action' else 'OFF'}", f"-DR2R_BUILD_ACTION_FEEDBACK_BENCH={'ON' if args.workload == 'action_feedback' else 'OFF'}", f"-DR2R_BUILD_EXECUTOR_BENCH={'ON' if args.workload == 'executor' else 'OFF'}", f"-DR2R_BUILD_STRING_BENCH={'ON' if args.workload == 'codec_strings' else 'OFF'}", f"-DR2R_BUILD_CALLBACK_GROUP_BENCH={'ON' if args.workload == 'callback_groups' else 'OFF'}", f"-DR2R_BUILD_ALLOCATION_PROBE={'ON' if args.with_probe else 'OFF'}"], root, env)
    cpp_binary = CPP_BINARIES[args.workload]
    jobs=str(build_jobs(env))
    command(["cmake", "--build", str(output / "cpp"), "--target", cpp_binary, "-j", jobs], root, env)
    if callback_cpp_snapshot is not None:require_unchanged_inputs(root,callback_cpp_snapshot)
    shutil.copy2(output / f"cpp/{cpp_binary}", output / "rclcpp-comparison")
    manifests["cpp_source_sha256"] = {str(path.relative_to(root)): hashlib.sha256(path.read_bytes()).hexdigest() for path in [root / "tools/bench_ros/CMakeLists.txt", root / "tools/bench_ros" / CPP_SOURCES[args.workload]]}
    if args.workload in {"codec", "codec_strings"}:
        manifests["cpp_source_sha256"].update({str(path.relative_to(root)): hashlib.sha256(path.read_bytes()).hexdigest() for path in sorted((root / "tools/bench_ros/codec_support").glob("*.hpp"))})
    if args.workload in {"action", "action_feedback"}:
        manifests["cpp_source_sha256"].update({str(path.relative_to(root)): hashlib.sha256(path.read_bytes()).hexdigest() for path in sorted((root / "tools/bench_ros/action_support").rglob("*.hpp"))})
        native_header = root / "tools/bench_ros/probe/probe.h"
        manifests["cpp_source_sha256"][str(native_header.relative_to(root))] = hashlib.sha256(native_header.read_bytes()).hexdigest()
    if args.workload == "callback_groups":
        dependencies=[*sorted((root/"tools/bench_ros/callback_group_support").rglob("*.hpp")),root/"tools/bench_ros/codec_support/probe.hpp",root/"tools/bench_ros/probe/probe.h"]
        manifests["cpp_source_sha256"].update({str(p.relative_to(root)):hashlib.sha256(p.read_bytes()).hexdigest() for p in dependencies})
    if args.with_probe:
        command(["cmake", "--build", str(output / "cpp"), "--target", "r2r_bench_probe", "-j", jobs], root, env)
        shutil.copy2(output / "cpp/libr2r_bench_probe.so", output / "libr2r_bench_probe.so")
        manifests["probe_source_sha256"] = {str(path.relative_to(root)): hashlib.sha256(path.read_bytes()).hexdigest() for path in sorted((root / "tools/bench_ros/probe").glob("probe.*"))}
    if args.workload == "callback_groups":
        binaries={"candidate":output/"r2r-candidate","cpp":output/"rclcpp-comparison"}
        if args.baseline_ref:binaries["baseline"]=output/"r2r-baseline"
        if args.with_probe:binaries["probe"]=output/"libr2r_bench_probe.so"
        manifests["binary_sha256"]={name:hashlib.sha256(p.read_bytes()).hexdigest() for name,p in binaries.items()}
    manifests["rustflags"] = env.get("RUSTFLAGS", "")
    manifests["cargo_release_opt_level"] = env.get("CARGO_PROFILE_RELEASE_OPT_LEVEL", "3")
    (output / "build-manifest.json").write_text(json.dumps(manifests, indent=2) + "\n")
    print(f"Comparison binaries and source manifest: {output}")


if __name__ == "__main__":
    main()
