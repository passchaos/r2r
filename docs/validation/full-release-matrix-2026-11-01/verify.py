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
        "jazzy_cyclonedds": (828, 24),
        "jazzy_fastdds": (828, 24),
        "kilted_cyclonedds": (834, 24),
        "kilted_fastdds": (834, 24),
        "humble_fastdds": (804, 24),
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
