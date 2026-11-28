"""Run the shared workflow's actual matrix planner against isolated projects."""
import json
import os
from pathlib import Path
import re
import tempfile
import unittest
from unittest.mock import patch

WORKFLOW = Path(__file__).resolve().parents[1] / '.github/workflows/github-ci.yml'


class AdvancedMatrixTests(unittest.TestCase):
    def plan(self, configs=(), runners='["ubuntu-latest"]', files=None, external=False):
        workflow = WORKFLOW.read_text()
        match = re.search(r"          python3 - <<'PYTHON'\n(.*?)\n          PYTHON", workflow, re.S)
        self.assertIsNotNone(match, 'shared workflow must plan before allocating suite runners')
        script = '\n'.join(line[10:] for line in match[1].splitlines())
        packages = [dict(id=str(i), metadata=cfg, dependencies=[])
                    for i, cfg in enumerate(configs)]
        metadata = dict(packages=packages, workspace_members=[] if external else [p['id'] for p in packages])
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            for name, content in (files or {}).items():
                path = root / name
                path.parent.mkdir(parents=True, exist_ok=True)
                path.write_text(content)
            output = root / 'outputs'
            with patch.dict(os.environ, GITHUB_WORKSPACE=directory, GITHUB_OUTPUT=str(output),
                            MEMORY_CHECK_OS=runners), patch('subprocess.check_output', return_value=json.dumps(metadata)):
                exec(compile(script, str(WORKFLOW), 'exec'), {})
            return json.loads(output.read_text().split('=', 1)[1])['include']

    def test_unconfigured_project_only_schedules_audit(self):
        self.assertEqual(self.plan(), [dict(suite='audit', os='ubuntu-latest')])

    def test_memory_checks_opt_in_and_default_to_ubuntu(self):
        rows = self.plan([{'rs-infra': {'miri': True, 'sanitizers': ['address']}}])
        self.assertEqual(rows, [dict(suite=s, os='ubuntu-latest')
                                for s in ['miri', 'address-sanitizer', 'audit']])

    def test_macos_requires_explicit_selection(self):
        rows = self.plan([{'rs-infra': {'miri': True}}], '["ubuntu-latest", "macos-26"]')
        self.assertEqual(rows[:2], [dict(suite='miri', os=os) for os in ['ubuntu-latest', 'macos-26']])
        self.assertEqual(len(rows), 3)

    def test_legacy_metadata_and_modern_precedence(self):
        self.assertEqual(len(self.plan([{'rs-ci': {'miri': True}}])), 2)
        self.assertEqual(len(self.plan([{'rs-ci': {'miri': True}, 'rs-infra': {}}])), 1)

    def test_dependencies_do_not_enable_workspace_checks(self):
        self.assertEqual(len(self.plan([{'rs-infra': {'miri': True}}], external=True)), 1)

    def test_optional_files_enable_only_their_suites(self):
        rows = self.plan(files={'.rs-ci-cross.toml': '', '.rs-ci-platform.toml': '',
                                'fuzz/Cargo.toml': '[package.metadata]\ncargo-fuzz = true\n'})
        self.assertEqual([r['suite'] for r in rows], ['fuzz', 'audit', 'cross', 'platform'])

    def test_invalid_package_configuration_fails_planning(self):
        for cfg in [{'miri': 'true'}, {'sanitizers': 'address'},
                    {'sanitizers': ['thread']}, {'sanitizers': ['address', 'address']}]:
            with self.subTest(cfg=cfg), self.assertRaises(ValueError):
                self.plan([{'rs-infra': cfg}])

    def test_fuzz_comments_do_not_enable_fuzz(self):
        rows = self.plan(files={'fuzz/Cargo.toml': '# cargo-fuzz = true\n[package]\nname = "fixture"\n'})
        self.assertEqual(rows, [dict(suite='audit', os='ubuntu-latest')])

    def test_execution_jobs_consume_the_planned_matrix(self):
        workflow = WORKFLOW.read_text()
        job = workflow.split('  advanced-verification:\n', 1)[1].split('  cross-targets:\n', 1)[0]
        self.assertIn('needs: [advanced-verification-plan]', job)
        self.assertIn('matrix: ${{ fromJSON(needs.advanced-verification-plan.outputs.matrix) }}', job)
        self.assertNotIn('suite-config', job)

    def test_invalid_runner_list_fails_planning(self):
        for runners in ['"ubuntu-latest"', '[]', '["windows-latest"]', '[1]']:
            with self.subTest(runners=runners), self.assertRaises(ValueError):
                self.plan(runners=runners)


if __name__ == '__main__':
    unittest.main()
