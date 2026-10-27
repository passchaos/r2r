"""Run independent Rust pairs with cohort-level alternating execution and quiescent peers."""
import json
import os
from pathlib import Path
import subprocess
import tempfile
import time
import uuid


def paired_cohorts(baseline, candidate, cpu_ping, cpu_echo, env, scenario, iterations,
                   warmup, count, repetition):
    processes, streams = {}, []
    with tempfile.TemporaryDirectory(prefix="r2r-paired-cohorts-") as temporary:
        directory = Path(temporary)
        try:
            for label, binary in [("baseline", baseline), ("rust", candidate)]:
                endpoint_env = dict(env, R2R_COHORT_CONTROL=str(directory), R2R_COHORT_LABEL=label)
                endpoint_env["ROS_DOMAIN_ID"] = str(int(env.get("ROS_DOMAIN_ID","0")) + (label == "rust"))
                prefix = "/r2r_cohort_" + uuid.uuid4().hex
                for role, cpu in [("echo", cpu_echo), ("ping", cpu_ping)]:
                    output = (directory / f"{label}-{role}.json").open("w")
                    errors = (directory / f"{label}-{role}.log").open("w")
                    streams.extend([output, errors])
                    processes[label, role] = subprocess.Popen(
                        ["taskset", "-c", str(cpu), str(binary), role, str(scenario), str(iterations),
                         str(warmup), prefix, str(count)], env=endpoint_env, stdout=output, stderr=errors)
            deadline = time.monotonic() + 120
            while not all((directory / f"{label}-{role}.ready").exists() for label,role in processes):
                for key,process in processes.items():
                    if process.poll() is not None:
                        raise RuntimeError(f"cohort endpoint {key} failed during initialization: {(directory / f'{key[0]}-{key[1]}.log').read_text()}")
                if time.monotonic() >= deadline:
                    raise RuntimeError("cohort endpoint initialization timed out")
                time.sleep(.001)
            for index in range(count):
                labels = ["baseline", "rust"] if (index + repetition) % 2 == 0 else ["rust", "baseline"]
                for label in labels:
                    (directory / f"{label}-{index}.go").write_text("")
                    deadline = time.monotonic() + 120
                    while not all((directory / f"{label}-{index}-{role}.done").exists() for role in ["ping", "echo"]):
                        for key, process in processes.items():
                            if process.poll() is not None:
                                raise RuntimeError(f"cohort endpoint {key} exited early: {(directory / f'{key[0]}-{key[1]}.log').read_text()}")
                        if time.monotonic() >= deadline:
                            raise RuntimeError(f"cohort {index}/{label} timed out")
                        time.sleep(.001)
            # Reporting/teardown must not contend with the other pair's final measured window.
            (directory / "render.go").write_text("")
            for key, process in processes.items():
                if process.wait(timeout=30):
                    raise RuntimeError(f"cohort endpoint {key} failed: {(directory / f'{key[0]}-{key[1]}.log').read_text()}")
            for stream in streams: stream.close()
            return {label: tuple(json.loads((directory / f"{label}-{role}.json").read_text()) for role in ["ping", "echo"])
                    for label in ["baseline", "rust"]}
        finally:
            for process in processes.values():
                if process.poll() is None:
                    process.terminate()
                    try: process.wait(timeout=5)
                    except subprocess.TimeoutExpired: process.kill(); process.wait()
            for stream in streams: stream.close()
