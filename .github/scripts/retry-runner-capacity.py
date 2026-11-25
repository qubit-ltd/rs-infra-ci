"""Retry macOS ARM64 memory checks cancelled before a hosted runner was assigned."""

import argparse
import json
import subprocess
from datetime import datetime, timedelta, timezone
from functools import partial
from urllib.parse import urlencode

SHARED_WORKFLOW = 'qubit-ltd/rs-infra-ci/.github/workflows/github-ci.yml'
CAPACITY_ANNOTATION = 'The job was not acquired by Runner of type hosted even after multiple attempts'
MAX_ATTEMPTS = 3
COOLDOWN = timedelta(minutes=15)
LOOKBACK = timedelta(days=1)


def parse_time(value):
    return datetime.fromisoformat(value.replace('Z', '+00:00'))


def eligible_run(run, branch, now):
    return (
        run['status'] == 'completed'
        and run['conclusion'] == 'failure'
        and run['run_attempt'] < MAX_ATTEMPTS
        and run['event'] == 'push'
        and run['head_branch'] == branch
        and run['path'] == '.github/workflows/ci.yml'
        and now - parse_time(run['updated_at']) >= COOLDOWN
        and any(workflow['path'].partition('@')[0] == SHARED_WORKFLOW
                for workflow in run.get('referenced_workflows') or [])
    )


def capacity_candidate(job):
    suite_name = job['name'].rsplit(' / ', 1)[-1]
    return (
        suite_name in {
            'Infrastructure suite (address-sanitizer on macos-26)',
            'Infrastructure suite (miri on macos-26)',
        }
        and 'macos-26' in job['labels']
        and job['status'] == 'completed'
        and job['conclusion'] == 'cancelled'
        and job['runner_id'] == 0
        and job['steps'] == []
    )


def scan_repository(api, now, dry_run):
    branch = api('')['default_branch']
    query = urlencode({
        'branch': branch, 'event': 'push', 'status': 'failure', 'per_page': 100,
        'created': '>=' + (now - LOOKBACK).strftime('%Y-%m-%dT%H:%M:%SZ'),
    })
    pages = api(f'actions/workflows/ci.yml/runs?{query}', paginate=True)
    for page in pages:
        for result in page['workflow_runs']:
            run_path = f"actions/runs/{result['id']}"
            run = api(run_path)
            if eligible_run(run, branch, now):
                retry_run(api, run_path, run, branch, now, dry_run)


def retry_run(api, run_path, run, branch, now, dry_run):
    # A targeted rerun can leave another cancelled matrix job in an earlier attempt.
    pages = api(f'{run_path}/jobs?filter=all&per_page=100', paginate=True)
    latest_jobs = {}
    for page in pages:
        for job in page['jobs']:
            latest = latest_jobs.get(job['name'])
            if latest is None or job_order(job) > job_order(latest):
                latest_jobs[job['name']] = job

    for job in sorted(latest_jobs.values(), key=lambda item: item['id']):
        if not capacity_candidate(job):
            continue
        job_id = job['id']
        annotations = api(f'check-runs/{job_id}/annotations?per_page=100', paginate=True)
        if not any(line['message'].strip() == CAPACITY_ANNOTATION
                   for page in annotations for line in page):
            continue

        current = api(run_path)
        if current['run_attempt'] != run['run_attempt'] or not eligible_run(current, branch, now):
            return
        if dry_run:
            print(f"Would rerun job {job_id} in run {run['id']} (attempt {run['run_attempt']})")
        else:
            api(f'actions/jobs/{job_id}/rerun', method='POST')
            print(f"Requested rerun of job {job_id} in run {run['id']} (attempt {run['run_attempt']})")
        return


def job_order(job):
    return job['completed_at'] or '', job['id']


def github_api(repository, path, *, method='GET', paginate=False):
    endpoint = f'repos/{repository}'
    if path:
        endpoint += '/' + path
    command = ['gh', 'api', '--method', method, endpoint]
    if paginate:
        command.extend(['--paginate', '--slurp'])
    result = subprocess.run(command, check=True, stdout=subprocess.PIPE, text=True)
    return json.loads(result.stdout) if result.stdout.strip() else None


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--owner', required=True)
    parser.add_argument('--repositories', required=True, help='Comma-separated repository names')
    parser.add_argument('--dry-run', action='store_true')
    args = parser.parse_args()
    now = datetime.now(timezone.utc)
    failed = []
    for name in args.repositories.split(','):
        repository = f'{args.owner}/{name}'
        print(f'Checking {repository}', flush=True)
        try:
            scan_repository(partial(github_api, repository), now, args.dry_run)
        except subprocess.CalledProcessError:
            failed.append(repository)
    if failed:
        parser.exit(1, f"GitHub API errors for: {', '.join(failed)}\n")


if __name__ == '__main__':
    main()
