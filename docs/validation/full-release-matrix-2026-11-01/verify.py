#!/usr/bin/env python3
"""Verify archived cross-release full library suite evidence."""

import gzip
import hashlib
import json
from pathlib import Path


HERE = Path(__file__).resolve().parent


def digest(path):
    return hashlib.sha256(path.read_bytes()).hexdigest()


def main():
    results = json.loads((HERE / "results.json").read_text())
    expected = {
        "jazzy_cyclonedds": (840, 27),
        "jazzy_fastdds": (840, 27),
        "kilted_cyclonedds": (846, 27),
        "kilted_fastdds": (846, 27),
        "humble_fastdds": (816, 27),
    }
    for name, gate in results["environments"].items():
        if (gate["passed"], gate["ignored"]) != expected[name]:
            raise RuntimeError(f"unexpected suite counts for {name}")
        artifact = HERE / gate["artifact"]
        if not artifact.is_file() or digest(artifact) != gate["artifact_sha256"]:
            raise RuntimeError(f"artifact digest mismatch: {gate['artifact']}")
        if hashlib.sha256(gzip.decompress(artifact.read_bytes())).hexdigest() != gate["uncompressed_sha256"]:
            raise RuntimeError(f"uncompressed log digest mismatch: {gate['artifact']}")
    print("cross-release full library suite evidence verified")


if __name__ == "__main__":
    main()
