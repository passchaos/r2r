"""Snapshot replacement must preserve existing bindings on partial failure."""

import importlib.util
from pathlib import Path
import tempfile
import unittest
from unittest.mock import patch


spec = importlib.util.spec_from_file_location("refresh_bindings", Path(__file__).with_name("refresh_bindings.py"))
refresh = importlib.util.module_from_spec(spec)
spec.loader.exec_module(refresh)


class SnapshotTransactionTests(unittest.TestCase):
    def prepare(self, root, missing=None):
        repository = root / "repository"
        candidate = root / "candidate"
        for crate in refresh.CRATES:
            current = repository / crate / "bindings"
            current.mkdir(parents=True)
            (current / "previous.txt").write_text(crate + " previous")
            if crate != missing:
                updated = candidate / crate / "bindings"
                updated.mkdir(parents=True)
                (updated / "updated.txt").write_text(crate + " updated")
        (repository / "unrelated.txt").write_text("preserve unrelated source")
        return repository, candidate

    def test_missing_later_candidate_preserves_every_prior_snapshot(self):
        with tempfile.TemporaryDirectory() as directory:
            repository, candidate = self.prepare(Path(directory), missing=refresh.CRATES[2])
            with patch.object(refresh, "ROOT", repository):
                with self.assertRaises(FileNotFoundError):
                    refresh.replace_bindings(candidate)
            for crate in refresh.CRATES:
                current = repository / crate / "bindings"
                self.assertEqual((current / "previous.txt").read_text(), crate + " previous")
                self.assertFalse((current / "updated.txt").exists())
            self.assertEqual((repository / "unrelated.txt").read_text(), "preserve unrelated source")

    def test_success_replaces_all_snapshots_and_removes_backup(self):
        with tempfile.TemporaryDirectory() as directory:
            repository, candidate = self.prepare(Path(directory))
            with patch.object(refresh, "ROOT", repository):
                refresh.replace_bindings(candidate)
            for crate in refresh.CRATES:
                current = repository / crate / "bindings"
                self.assertEqual((current / "updated.txt").read_text(), crate + " updated")
                self.assertFalse((current / "previous.txt").exists())
            self.assertFalse((candidate / "previous").exists())
            self.assertFalse(list(repository.glob("*/.r2r-bindings-*")))
            self.assertEqual((repository / "unrelated.txt").read_text(), "preserve unrelated source")

    def test_install_failure_rolls_back_already_replaced_snapshots(self):
        with tempfile.TemporaryDirectory() as directory:
            repository, candidate = self.prepare(Path(directory))
            failed_destination = repository / refresh.CRATES[2] / "bindings"
            rename = Path.rename

            def fail_install(path, destination):
                if path.name == "next" and Path(destination) == failed_destination:
                    raise OSError("injected installation failure")
                return rename(path, destination)

            with patch.object(refresh, "ROOT", repository), patch.object(Path, "rename", fail_install):
                with self.assertRaisesRegex(OSError, "injected installation failure"):
                    refresh.replace_bindings(candidate)
            for crate in refresh.CRATES:
                current = repository / crate / "bindings"
                self.assertEqual((current / "previous.txt").read_text(), crate + " previous")
                self.assertFalse((current / "updated.txt").exists())
                self.assertTrue((candidate / crate / "bindings/updated.txt").is_file())
            self.assertFalse(list(repository.glob("*/.r2r-bindings-*")))

    def test_candidate_cannot_drop_a_previously_documented_package(self):
        with tempfile.TemporaryDirectory() as directory:
            repository, candidate = self.prepare(Path(directory))
            previous = repository / "r2r/bindings/_r2r_generated_msgs.rs"
            previous.write_text("pub mod previous_api {}")
            generated = candidate / "r2r/bindings/_r2r_generated_msgs.rs"
            generated.write_text("pub mod requested_api {}")
            with patch.object(refresh, "ROOT", repository):
                with self.assertRaisesRegex(RuntimeError, "previous_api"):
                    refresh.verify_candidate(candidate, ["requested_api"])


if __name__ == "__main__":
    unittest.main()
