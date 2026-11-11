"""Exercise the CI entrypoint with an isolated, offline bootstrap."""
import os
from pathlib import Path
import shutil
import signal
import subprocess
import tempfile
import time
import unittest

ENTRYPOINT = Path(__file__).resolve().parents[1] / '.infra/bin/ci-check.sh'


class CiCheckLockTests(unittest.TestCase):
    def setUp(self):
        self.temp = tempfile.TemporaryDirectory()
        self.addCleanup(self.temp.cleanup)

    def project(self, name):
        root = Path(self.temp.name) / name
        (root / '.infra/bin').mkdir(parents=True)
        shutil.copyfile(ENTRYPOINT, root / '.infra/bin/ci-check.sh')
        bootstrap = root / '.infra/bootstrap.sh'
        bootstrap.write_text('''#!/usr/bin/env bash
set -eu
printf 'started\\n' >> "$(dirname "$0")/calls"
if [[ ${2:-} == cleanup ]]; then
    trap 'touch "$(dirname "$0")/ready"; sleep 30' EXIT
    exit 0
fi
if [[ ${2:-} == hold ]]; then
    touch "$(dirname "$0")/ready"
    sleep 30
fi
exit "${2:-0}"
''')
        bootstrap.chmod(0o755)
        return root

    def run_ci(self, root, *args):
        return subprocess.run(['bash', str(root / '.infra/bin/ci-check.sh'), *args],
                              capture_output=True, text=True, timeout=5)

    def hold(self, root, mode='hold'):
        proc = subprocess.Popen(['bash', str(root / '.infra/bin/ci-check.sh'), mode],
                                start_new_session=True, stdout=subprocess.DEVNULL,
                                stderr=subprocess.DEVNULL)
        def stop():
            if proc.poll() is None:
                os.killpg(proc.pid, signal.SIGKILL)
            proc.wait(timeout=5)
        self.addCleanup(stop)
        deadline = time.monotonic() + 5
        while not (root / '.infra/ready').exists():
            if proc.poll() is not None or time.monotonic() >= deadline:
                self.fail('first CI did not start')
            time.sleep(0.02)
        return proc

    def test_duplicate_is_rejected_before_bootstrap_and_other_crates_can_run(self):
        root = self.project('crate-a')
        self.hold(root)
        result = self.run_ci(root)
        self.assertNotEqual(result.returncode, 0)
        self.assertIn('already running', result.stderr)
        self.assertEqual((root / '.infra/calls').read_text(), 'started\n')
        self.assertEqual(self.run_ci(self.project('crate-b')).returncode, 0)

    def test_lock_covers_exit_cleanup(self):
        root = self.project('crate-a')
        self.hold(root, 'cleanup')
        result = self.run_ci(root)
        self.assertNotEqual(result.returncode, 0)
        self.assertIn('already running', result.stderr)
        self.assertEqual((root / '.infra/calls').read_text(), 'started\n')

    def test_lock_is_released_after_failure(self):
        root = self.project('crate-a')
        self.assertEqual(self.run_ci(root, '7').returncode, 7)
        self.assertEqual(self.run_ci(root).returncode, 0)

    def test_lock_is_released_after_kill(self):
        root = self.project('crate-a')
        proc = self.hold(root)
        os.killpg(proc.pid, signal.SIGKILL)
        proc.wait(timeout=5)
        self.assertEqual(self.run_ci(root).returncode, 0)


if __name__ == '__main__':
    unittest.main()
