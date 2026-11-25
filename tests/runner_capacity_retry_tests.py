import importlib.util
import unittest
from datetime import datetime, timezone
from pathlib import Path

SCRIPT = Path(__file__).resolve().parents[1] / '.github/scripts/retry-runner-capacity.py'
SPEC = importlib.util.spec_from_file_location('retry_runner_capacity', SCRIPT)
retry = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(retry)

NOW = datetime(2026, 11, 25, 5, tzinfo=timezone.utc)
RUN = {
    'id': 45426033838, 'status': 'completed', 'conclusion': 'failure',
    'run_attempt': 2, 'event': 'push', 'head_branch': 'main',
    'path': '.github/workflows/ci.yml', 'updated_at': '2026-11-25T04:40:00Z',
    'referenced_workflows': [
        {'path': 'qubit-ltd/rs-infra-ci/.github/workflows/github-ci.yml@main'},
    ],
}
JOB = {
    'id': 138579745747,
    'name': 'rust-ci / Infrastructure suite (address-sanitizer on macos-26)',
    'labels': ['macos-26'], 'status': 'completed', 'conclusion': 'cancelled',
    'runner_id': 0, 'steps': [], 'completed_at': '2026-11-25T04:40:00Z',
}
ANNOTATION = {'message': 'The job was not acquired by Runner of type hosted even after multiple attempts'}


class RetryTests(unittest.TestCase):
    def test_capacity_cancelled_job_is_retried_in_place(self):
        self.assertEqual(self.scan(), ['actions/jobs/138579745747/rerun'])

    def test_test_failure_and_missing_capacity_evidence_are_skipped(self):
        for job, annotations in [
            (dict(JOB, conclusion='failure', runner_id=21), [ANNOTATION]),
            (dict(JOB, steps=[{'name': 'Set up job'}]), [ANNOTATION]),
            (dict(JOB, labels=['macos-26-intel']), [ANNOTATION]),
            (JOB, [{'message': 'Cancelled by user'}]),
        ]:
            with self.subTest(job=job, annotations=annotations):
                self.assertEqual(self.scan(jobs=[job], annotations=annotations), [])

    def test_latest_success_supersedes_earlier_cancellation(self):
        success = dict(JOB, id=JOB['id'] + 1, conclusion='success',
                       completed_at='2026-11-25T04:41:00Z')
        self.assertEqual(self.scan(jobs=[success, JOB]), [])

    def test_only_one_job_per_run_is_retried(self):
        miri = dict(JOB, id=JOB['id'] - 1, name=JOB['name'].replace('address-sanitizer', 'miri'))
        self.assertEqual(len(self.scan(jobs=[JOB, miri])), 1)

    def test_active_run_attempt_limit_cooldown_and_unrelated_workflow_are_skipped(self):
        for changes in [
            {'status': 'in_progress'}, {'run_attempt': 3},
            {'updated_at': '2026-11-25T04:59:00Z'}, {'referenced_workflows': []},
            {'event': 'pull_request'}, {'head_branch': 'dev'},
        ]:
            with self.subTest(changes=changes):
                self.assertEqual(self.scan(run=dict(RUN, **changes)), [])

    def test_manual_rerun_between_scan_and_write_is_respected(self):
        self.assertEqual(self.scan(recheck=dict(RUN, status='queued', run_attempt=3)), [])

    def test_dry_run_does_not_write(self):
        self.assertEqual(self.scan(dry_run=True), [])

    def scan(self, run=RUN, jobs=None, annotations=None, recheck=None, dry_run=False):
        posts = []
        run_path = f"actions/runs/{RUN['id']}"
        run_reads = iter([run, recheck or run])

        def api(path, *, method='GET', paginate=False):
            if method == 'POST':
                posts.append(path)
                return None
            if path == '':
                return {'default_branch': 'main'}
            if path.startswith('actions/workflows/ci.yml/runs?'):
                return [{'workflow_runs': [{'id': RUN['id']}]}]
            if path == run_path:
                return next(run_reads)
            if path == run_path + '/jobs?filter=all&per_page=100':
                return [{'jobs': jobs if jobs is not None else [JOB]}]
            if path.startswith('check-runs/') and path.endswith('/annotations?per_page=100'):
                return [annotations if annotations is not None else [ANNOTATION]]
            self.fail(f'Unexpected API path: {path}')

        retry.scan_repository(api, NOW, dry_run)
        return posts


if __name__ == '__main__':
    unittest.main()
