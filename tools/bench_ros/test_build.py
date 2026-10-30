from pathlib import Path
import tempfile
import unittest
from build import registry_lock_entries,snapshot_inputs,require_unchanged_inputs


class DependencyLockValidation(unittest.TestCase):
    def test_registry_identity_ignores_workspace_manifest_metadata(self):
        with tempfile.TemporaryDirectory() as directory:
            path=Path(directory)/'Cargo.lock'
            path.write_text('''version = 4
[[package]]
name = "r2r_msg_gen"
version = "0.9.7"
dependencies = ["serde"]
[[package]]
name = "serde"
version = "1.0.1"
source = "registry+https://example.invalid/index"
checksum = "abc"
''')
            expected={('serde','1.0.1','registry+https://example.invalid/index')}
            self.assertEqual(registry_lock_entries(path),expected)
            path.write_text(path.read_text().replace('1.0.1','1.0.2'))
            self.assertFalse(registry_lock_entries(path).issubset(expected))


class BuildInputValidation(unittest.TestCase):
    def test_modified_or_missing_input_cannot_be_attested(self):
        with tempfile.TemporaryDirectory() as directory:
            root=Path(directory);source=root/'callback.rs';source.write_text('original')
            snapshot=snapshot_inputs(root,[source]);require_unchanged_inputs(root,snapshot)
            source.write_text('changed')
            with self.assertRaisesRegex(RuntimeError,'changed during compilation'):require_unchanged_inputs(root,snapshot)
            source.unlink()
            with self.assertRaisesRegex(RuntimeError,'changed during compilation'):require_unchanged_inputs(root,snapshot)
