#!/usr/bin/env python3
"""Compile/run Linux-glibc probe oracles, including late writers and dynamic TLS loading."""
import argparse
import hashlib
import json
import os
from pathlib import Path
import platform
import subprocess


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--output-dir", type=Path, default=Path("target/probe-validation"))
    args = parser.parse_args()
    if platform.system() != "Linux" or platform.libc_ver()[0] != "glibc":
        parser.error("this probe requires Linux/glibc")
    source = Path(__file__).resolve().parent
    output = args.output_dir.resolve()
    output.mkdir(parents=True, exist_ok=True)
    flags = ["-std=c11", "-O2", "-g", "-Wall", "-Wextra", "-Werror"]
    reports = []
    for mode, defines in [("normal", []), ("epoch_testing", ["-DR2R_BENCH_PROBE_TESTING"])]:
        directory = output / mode
        directory.mkdir(exist_ok=True)
        library = directory / "libr2r_bench_probe.so"
        plugin = directory / "test_dynamic.so"
        executable = directory / "test_probe"
        commands = [
            ["cc", *flags, *defines, "-fPIC", "-shared", str(source / "probe.c"), "-ldl", "-pthread", "-o", str(library)],
            ["cc", *flags, "-fPIC", "-shared", str(source / "test_dynamic.c"), "-o", str(plugin)],
            ["cc", *flags, *defines, str(source / "test_probe.c"), "-L" + str(directory), "-lr2r_bench_probe",
             "-Wl,-rpath," + str(directory), "-ldl", "-pthread", "-o", str(executable)],
        ]
        for command in commands:
            subprocess.run(command, check=True)
        env = os.environ.copy()
        env["LD_PRELOAD"] = str(library) + (":" + env["LD_PRELOAD"] if env.get("LD_PRELOAD") else "")
        result = subprocess.run([str(executable), str(plugin)], env=env, capture_output=True,
                                text=True, timeout=30, check=True)
        print(mode + ": " + result.stdout.strip())
        reports.append({"mode": mode, "passed": True, "stdout": result.stdout,
                        "library_sha256": hashlib.sha256(library.read_bytes()).hexdigest(),
                        "executable_sha256": hashlib.sha256(executable.read_bytes()).hexdigest()})
    report = {"compiler": subprocess.check_output(["cc", "--version"], text=True).splitlines()[0],
              "platform": platform.platform(), "libc": platform.libc_ver(), "compile_flags": flags,
              "source_sha256": {path.name: hashlib.sha256(path.read_bytes()).hexdigest()
                                for path in [source / "probe.c", source / "probe.h", source / "test_probe.c", source / "test_dynamic.c"]},
              "oracles": reports}
    (output / "report.json").write_text(json.dumps(report, indent=2) + "\n")


if __name__ == "__main__":
    main()
