import copy
import unittest
import tempfile
import json
from pathlib import Path
from type_descriptions import validate, verify


class DescriptionOracleBoundaries(unittest.TestCase):
    def test_language_matrix_hash_sources_and_omission_are_required(self):
        records = {}
        for server, client in [("rust", "rclcpp"), ("cpp", "r2r")]:
            records[server + "_server"] = {
                "server": {"kind": "ready", "role": "server", "hash": "RIHS01_" + "ab" * 32},
                "client": {"kind": "result", "implementation": client, "type_name": "std_msgs/msg/UInt64",
                           "hash": "RIHS01_" + "ab" * 32, "fields": 1, "sources": True, "omission": True, "exact_source": False, "encoding":"msg", "source_hex":"68656c6c6f"},
            }
        validate(records)
        with tempfile.TemporaryDirectory() as directory:
            path = Path(directory) / "native-source.json"
            report = {"passed":True, "records":records, "installed_source_hex": {name:"68656c6c6f0a" for name in records}, "exact_source_gate":{"passed":False}}
            path.write_text(json.dumps(report))
            verify(path, current=False)
            with self.assertRaisesRegex(RuntimeError, "exact installed-source gate failed"):
                verify(path, current=False, exact=True)
            report["exact_source_gate"]["passed"] = True
            path.write_text(json.dumps(report))
            with self.assertRaisesRegex(RuntimeError, "fidelity gate mismatch"):
                verify(path, current=False)
        missing = copy.deepcopy(records); del missing["cpp_server"]
        with self.assertRaisesRegex(RuntimeError, "incomplete"): validate(missing)
        for field, value in [("sources", False), ("omission", False), ("fields", 0), ("hash", "RIHS01_" + "cd" * 32)]:
            changed = copy.deepcopy(records); changed["rust_server"]["client"][field] = value
            with self.assertRaisesRegex(RuntimeError, "identity/source"): validate(changed)
        changed = copy.deepcopy(records); changed["rust_server"]["server"]["hash"] = "invalid"
        with self.assertRaisesRegex(RuntimeError, "invalid native RIHS"): validate(changed)


if __name__ == "__main__": unittest.main()
