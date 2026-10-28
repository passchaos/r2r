#!/usr/bin/env python3
"""Generate, validate and replace the documentation snapshot as one transaction."""

import importlib.util
import os
from pathlib import Path
import re
import shutil
import subprocess
import tempfile


ROOT = Path(__file__).resolve().parents[1]
CRATES = ("r2r", "r2r_rcl", "r2r_actions", "r2r_msg_gen")


def exported_modules(bindings):
    return set(re.findall(r"pub mod (\w+)", (bindings / "_r2r_generated_msgs.rs").read_text()))


def verify_candidate(candidate, packages):
    messages = candidate / "r2r" / "bindings"
    modules = exported_modules(messages)
    required = set(packages) | exported_modules(ROOT / "r2r" / "bindings")
    missing = required - modules
    if missing:
        raise RuntimeError(f"snapshot lost documented packages: {sorted(missing)}")
    filenames = (messages / "files.txt").read_text().splitlines()
    if len(filenames) != len(set(filenames)):
        raise RuntimeError("duplicate entries in generated snapshot file list")
    if "_r2r_generated_cfg.txt" not in filenames:
        raise RuntimeError("missing documentation interface availability")
    for name in filenames:
        if Path(name).name != name or not (messages / name).is_file():
            raise RuntimeError(f"invalid or missing snapshot file: {name}")
    for crate in CRATES:
        if not any((candidate / crate / "bindings").iterdir()):
            raise RuntimeError(f"empty snapshot for {crate}")


def replace_bindings(candidate):
    # Prepare each replacement beside its destination. CARGO_TARGET_DIR may
    # live on another filesystem (for example a RAM-backed build directory).
    # Only same-filesystem renames participate in the replacement/rollback.
    prepared = []
    moved = []
    try:
        for crate in CRATES:
            current = ROOT / crate / "bindings"
            stage = Path(tempfile.mkdtemp(prefix=".r2r-bindings-", dir=current.parent))
            prepared.append((current, stage))
            shutil.copytree(candidate / crate / "bindings", stage / "next")
        for current, stage in prepared:
            previous = stage / "previous"
            current.rename(previous)
            moved.append((current, stage))
            (stage / "next").rename(current)
    except BaseException:
        for current, stage in reversed(moved):
            if current.exists():
                current.rename(stage / "next")
            (stage / "previous").rename(current)
        for _, stage in prepared:
            shutil.rmtree(stage)
        raise
    for _, stage in prepared:
        shutil.rmtree(stage)


def main():
    distro = (ROOT / "r2r_common" / "documentation_distro.txt").read_text().strip()
    if os.environ.get("ROS_DISTRO") != distro:
        raise RuntimeError(f"source ROS {distro} before refreshing its documentation snapshot")
    packages = (ROOT / "tools" / "documentation_packages.txt").read_text().splitlines()
    prefixes = [Path(p) for p in os.environ.get("AMENT_PREFIX_PATH", "").split(os.pathsep) if p]
    missing = [
        package for package in packages
        if not any((prefix / "share/ament_index/resource_index/rosidl_interfaces" / package).is_file()
                   for prefix in prefixes)
    ]
    if missing:
        raise RuntimeError(f"source the required documentation packages: {missing}")

    target = Path(os.environ.get("CARGO_TARGET_DIR", ROOT / "target"))
    if not target.is_absolute():
        target = ROOT / target
    target = target.resolve()
    staging = target / "documentation-bindings"
    staging.mkdir(parents=True, exist_ok=True)
    candidate = Path(tempfile.mkdtemp(prefix="candidate-", dir=staging))
    environment = os.environ.copy()
    environment["CARGO_TARGET_DIR"] = str(target)
    environment["R2R_BINDINGS_DIR"] = str(candidate)
    environment["IDL_PACKAGE_FILTER"] = ";".join(packages)
    environment.pop("CMAKE_IDL_PACKAGES", None)
    print(f"Generating {distro} documentation snapshot at {candidate}", flush=True)
    subprocess.run(
        ["cargo", "build", "--workspace", "--features", "save-bindgen"],
        cwd=ROOT, env=environment, check=True,
    )
    verify_candidate(candidate, packages)

    spec = importlib.util.spec_from_file_location("check_documentation", ROOT / "tests/check_documentation.py")
    checker = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(checker)
    documentation_environment = checker.documentation_environment()
    documentation_environment["R2R_BINDINGS_DIR"] = str(candidate)
    documentation_environment["CARGO_TARGET_DIR"] = str(staging / "validation")
    checker.check(documentation_environment)
    replace_bindings(candidate)
    shutil.rmtree(candidate)
    print("Validated documentation snapshot installed; existing Cargo artifacts retained")


if __name__ == "__main__":
    main()
