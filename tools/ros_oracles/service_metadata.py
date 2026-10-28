#!/usr/bin/env python3
"""Build and run a native RCL response-metadata initialization oracle."""

import json
import hashlib
import os
from pathlib import Path
import subprocess
import tempfile
import xml.etree.ElementTree as ET


source = Path(__file__).with_suffix(".cpp")
prefixes = [Path(p) for p in os.environ["AMENT_PREFIX_PATH"].split(os.pathsep) if p]
with tempfile.TemporaryDirectory(prefix="r2r-service-metadata-") as directory:
    binary = Path(directory) / "oracle"
    arguments = ["c++", "-std=c++17", "-O2", str(source), "-o", str(binary)]
    for prefix in prefixes:
        arguments += ["-I" + str(prefix / "include"), "-L" + str(prefix / "lib")]
        arguments += ["-I" + str(path) for path in sorted((prefix / "include").glob("*")) if path.is_dir()]
    arguments += ["-lrcl", "-lrcutils", "-lrmw", "-lrmw_implementation", "-lexample_interfaces__rosidl_typesupport_c"]
    subprocess.run(arguments, check=True)
    result = subprocess.run([str(binary)], check=True, text=True, stdout=subprocess.PIPE)
    report = json.loads(result.stdout)
    assert len(report["samples"]) == 3
    assert [sample["fill"] for sample in report["samples"]] == [0, 165, 90]
    assert len({sample["sequence"] for sample in report["samples"]}) == 3
    written = []
    for byte in range(16):
        values = [sample["response_guid"][byte] for sample in report["samples"]]
        if len(set(values)) == 1:
            written.append(True)
        elif values == [sample["fill"] for sample in report["samples"]]:
            written.append(False)
        else:
            raise RuntimeError(f"response identity changed with buffer initialization at byte {byte}: {values}")
    report["response_guid_written_mask"] = written
    report["ros_distro"] = os.environ["ROS_DISTRO"]
    report["compiler"] = subprocess.check_output(["c++", "--version"], text=True).splitlines()[0]
    report["source_sha256"] = {
        source.name: hashlib.sha256(source.read_bytes()).hexdigest(),
        Path(__file__).name: hashlib.sha256(Path(__file__).read_bytes()).hexdigest(),
    }
    report["package_versions"] = {}
    for package in ["rcl", "rmw", "example_interfaces", report["rmw"]]:
        manifest = next((prefix / "share" / package / "package.xml" for prefix in prefixes
                         if (prefix / "share" / package / "package.xml").is_file()), None)
        if manifest is None:
            raise RuntimeError(f"missing native oracle package metadata: {package}")
        report["package_versions"][package] = ET.parse(manifest).findtext("version")
    print(json.dumps(report, indent=2))
