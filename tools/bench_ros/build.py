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


def command(arguments, cwd, env):
    result = subprocess.run(arguments, cwd=cwd, env=env, text=True, stdout=subprocess.PIPE, check=True)
    return result.stdout


def rust_binary(source, target, destination, env, workload="codec"):
    build_env = env.copy()
    build_env["CARGO_TARGET_DIR"] = str(target)
    benchmark = "ros_comparison" if workload == "codec" else "pubsub_roundtrip"
    output = command(["cargo", "build", "--locked", "--release", "-p", "r2r", "--bench", benchmark, "--message-format=json"], source, build_env)
    artifacts = [json.loads(line) for line in output.splitlines() if line.startswith("{")]
    executable = next(item["executable"] for item in artifacts if item.get("reason") == "compiler-artifact" and item.get("target", {}).get("name") == benchmark and item.get("executable"))
    shutil.copy2(executable, destination)
    return {str(path.relative_to(source)): hashlib.sha256(path.read_bytes()).hexdigest() for path in [source / f"r2r/benches/{benchmark}.rs", source / "r2r/src/nodes.rs", source / "r2r/src/subscribers.rs", source / "r2r/src/publishers.rs", source / "r2r_rcl/src/lib.rs", source / "r2r_msg_gen/src/lib.rs", source / "Cargo.lock"]}


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--baseline-ref", help="Git revision supplying the baseline library code")
    parser.add_argument("--workload", choices=["codec", "transport"], default="codec")
    parser.add_argument("--output-dir", type=Path, default=Path("target/ros-comparison"))
    args = parser.parse_args()
    root = Path(__file__).resolve().parents[2]
    output = args.output_dir.resolve()
    output.mkdir(parents=True, exist_ok=True)
    env = os.environ.copy()
    manifests = {}
    manifests["workload"] = args.workload
    benchmark = "ros_comparison" if args.workload == "codec" else "pubsub_roundtrip"
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
            shutil.copy2(root / "Cargo.lock", source / "Cargo.lock")
            manifest = source / "r2r/Cargo.toml"
            if f'name = "{benchmark}"' not in manifest.read_text():
                manifest.write_text(manifest.read_text() + f'\n[[bench]]\nname = "{benchmark}"\nharness = false\n')
            manifests["baseline"] = rust_binary(source, output / "baseline-cargo", output / "r2r-baseline", env, args.workload)
        manifests["baseline_ref"] = revision
    ros_prefix = f"/opt/ros/{env['ROS_DISTRO']}"
    command(["cmake", "-S", str(root / "tools/bench_ros"), "-B", str(output / "cpp"), "-DCMAKE_BUILD_TYPE=Release", f"-DCMAKE_PREFIX_PATH={ros_prefix}", "-DPython3_EXECUTABLE=/usr/bin/python3"], root, env)
    cpp_binary = "rclcpp_comparison" if args.workload == "codec" else "rclcpp_roundtrip"
    command(["cmake", "--build", str(output / "cpp"), "--target", cpp_binary, "-j", "4"], root, env)
    shutil.copy2(output / f"cpp/{cpp_binary}", output / "rclcpp-comparison")
    manifests["cpp_source_sha256"] = {str(path.relative_to(root)): hashlib.sha256(path.read_bytes()).hexdigest() for path in [root / "tools/bench_ros/CMakeLists.txt", root / ("tools/bench_ros/comparison.cpp" if args.workload == "codec" else "tools/bench_ros/roundtrip.cpp")]}
    manifests["rustflags"] = env.get("RUSTFLAGS", "")
    manifests["cargo_release_opt_level"] = env.get("CARGO_PROFILE_RELEASE_OPT_LEVEL", "3")
    (output / "build-manifest.json").write_text(json.dumps(manifests, indent=2) + "\n")
    print(f"Comparison binaries and source manifest: {output}")


if __name__ == "__main__":
    main()
