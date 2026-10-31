from pathlib import Path
import tempfile
import unittest
from build import (build_jobs, pubsub_receive_configuration, registry_lock_entries,
                   snapshot_inputs, require_unchanged_inputs)


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
    def test_pubsub_receive_cfg_requires_the_requested_public_api(self):
        with tempfile.TemporaryDirectory() as directory:
            root=Path(directory);source=root/'r2r/src/nodes/subscription_api.rs'
            source.parent.mkdir(parents=True);source.write_text('')
            self.assertEqual(pubsub_receive_configuration(root,'reusable_typed'),('stream',''))
            self.assertEqual(pubsub_receive_configuration(root,'batch_typed'),('stream',''))
            source.write_text('pub fn subscribe_reusable() {}\npub fn subscribe_native_callback() {}\npub fn subscribe_batches() {}')
            self.assertEqual(pubsub_receive_configuration(root,'reusable_typed'),
                             ('reusable_typed',' --cfg r2r_bench_reusable_typed'))
            self.assertEqual(pubsub_receive_configuration(root,'native_callback'),
                             ('native_callback',' --cfg r2r_bench_native_callbacks'))
            self.assertEqual(pubsub_receive_configuration(root,'batch_typed'),
                             ('batch_typed',' --cfg r2r_bench_batch_typed'))
            with self.assertRaisesRegex(RuntimeError,'invalid pubsub receive path'):
                pubsub_receive_configuration(root,'unknown')

    def test_modified_or_missing_input_cannot_be_attested(self):
        with tempfile.TemporaryDirectory() as directory:
            root=Path(directory);source=root/'callback.rs';source.write_text('original')
            snapshot=snapshot_inputs(root,[source]);require_unchanged_inputs(root,snapshot)
            source.write_text('changed')
            with self.assertRaisesRegex(RuntimeError,'changed during compilation'):require_unchanged_inputs(root,snapshot)
            source.unlink()
            with self.assertRaisesRegex(RuntimeError,'changed during compilation'):require_unchanged_inputs(root,snapshot)

    def test_build_parallelism_honors_explicit_and_cargo_limits(self):
        self.assertEqual(build_jobs({}),4)
        self.assertEqual(build_jobs({'CARGO_BUILD_JOBS':'2'}),2)
        self.assertEqual(build_jobs({'CARGO_BUILD_JOBS':'2','R2R_BUILD_JOBS':'1'}),1)
        for value in ['0','-1','many']:
            with self.assertRaisesRegex(RuntimeError,'invalid build parallelism'):
                build_jobs({'R2R_BUILD_JOBS':value})
