const test = require('node:test');
const assert = require('node:assert/strict');
const { isAcquisitionFailure, retryRelease } = require('./retry_release_infrastructure.cjs');

const capacity = { annotation_level: 'failure', message: 'The job was not acquired by Runner of type hosted even after multiple attempts' };
const job = { name: 'Test macos-arm64', conclusion: 'cancelled', steps: [], check_run_url: 'https://api.github.com/repos/o/r/check-runs/123' };

function fixture({ attempt = 1, jobs = [job], annotations = [capacity], current = {} } = {}) {
  const run = { id: 42, run_attempt: attempt, status: 'completed', conclusion: 'failure', html_url: 'https://github.com/o/r/actions/runs/42' };
  const calls = { retries: [], delays: [], failures: [], summaries: 0 };
  let reads = 0;
  const github = {
    rest: {
      actions: {
        getWorkflowRun: async () => ({ data: { ...run, ...(reads++ ? current : {}) } }),
        listJobsForWorkflowRun: 'jobs',
        reRunWorkflowFailedJobs: async args => calls.retries.push(args),
      },
      checks: { listAnnotations: 'annotations' },
    },
    paginate: async (method, args) => {
      assert.equal(args.owner, 'o');
      assert.equal(args.repo, 'r');
      if (method === 'jobs') { assert.equal(args.filter, 'latest'); return jobs; }
      assert.equal(args.check_run_id, 123);
      return annotations;
    },
  };
  const summary = { addHeading() { return this; }, addRaw() { return this; }, async write() { calls.summaries++; } };
  return { calls, args: { github, context: { repo: { owner: 'o', repo: 'r' }, payload: { workflow_run: run } },
    core: { info() {}, setFailed: message => calls.failures.push(message), summary },
    delay: async ms => calls.delays.push(ms) } };
}

test('recognizes the actual hosted-runner acquisition annotation', () => {
  assert.equal(isAcquisitionFailure(job, [capacity]), true);
  assert.equal(isAcquisitionFailure({ ...job, conclusion: 'failure' }, [capacity]), true);
  assert.equal(isAcquisitionFailure(job, [{ ...capacity, annotation_level: 'notice' }]), false);
  assert.equal(isAcquisitionFailure(job, [{ ...capacity, message: 'Due to capacity constraints, jobs may experience longer queue times.' }]), false);
});

test('retries unfinished jobs once after cooldown, preserving successful and skipped jobs', async () => {
  const { calls, args } = fixture({ jobs: [job, { conclusion: 'success' }, { conclusion: 'skipped' }] });
  await retryRelease(args);
  assert.deepEqual(calls.retries, [{ owner: 'o', repo: 'r', run_id: 42 }]);
  assert.deepEqual(calls.delays, [60_000]);
  assert.equal(calls.summaries, 1);
});

test('does not retry compilation, test, or execution timeout failures', async () => {
  for (const message of ['test failed', 'compilation failed', 'The job has exceeded the maximum execution time']) {
    const { calls, args } = fixture({ jobs: [{ ...job, conclusion: 'failure', steps: [{ conclusion: 'failure' }] }], annotations: [{ ...capacity, message }] });
    await retryRelease(args);
    assert.equal(calls.retries.length, 0);
    assert.equal(calls.delays.length, 0);
  }
});

test('does not retry manual cancellations or mixed code and infrastructure failures', async () => {
  for (const jobs of [[job], [job, { ...job, conclusion: 'failure', steps: [{ conclusion: 'failure' }] }]]) {
    const { calls, args } = fixture({ jobs, annotations: jobs.length === 1 ? [] : [capacity] });
    await retryRelease(args);
    assert.equal(calls.retries.length, 0);
  }
});

test('allows at most three retries and reports persistent capacity failure', async () => {
  const third = fixture({ attempt: 3 });
  await retryRelease(third.args);
  assert.equal(third.calls.retries.length, 1);
  const fourth = fixture({ attempt: 4 });
  await retryRelease(fourth.args);
  assert.equal(fourth.calls.retries.length, 0);
  assert.match(fourth.calls.failures[0], /retry limit reached/);
});

test('ignores stale events and runs restarted or cancelled during cooldown', async () => {
  for (const current of [{ status: 'in_progress' }, { run_attempt: 2 }, { conclusion: 'cancelled' }]) {
    const { calls, args } = fixture({ current });
    await retryRelease(args);
    assert.equal(calls.retries.length, 0);
  }
  const { calls, args } = fixture();
  args.context.payload.workflow_run = { ...args.context.payload.workflow_run, run_attempt: 0 };
  await retryRelease(args);
  assert.equal(calls.delays.length, 0);
  assert.equal(calls.retries.length, 0);
});
