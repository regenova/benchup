// Only retry jobs that GitHub could not assign to a hosted runner.
// Test failures, compilation failures, execution timeouts, and manual
// cancellations must remain visible for a person to investigate.
const MAX_ATTEMPTS = 4; // Initial attempt plus at most three automatic retries.
const ACQUISITION_FAILURE = /The job was not acquired by Runner of type hosted even after multiple attempts/i;

function isAcquisitionFailure(job, annotations) {
  return ['failure', 'cancelled'].includes(job.conclusion)
    && (job.steps ?? []).length === 0
    && annotations.some(annotation => annotation.annotation_level === 'failure'
      && ACQUISITION_FAILURE.test(annotation.message ?? ''));
}

async function retryRelease({ github, context, core, delay = ms => new Promise(resolve => setTimeout(resolve, ms)) }) {
  const eventRun = context.payload.workflow_run;
  const repo = context.repo;
  const { data: run } = await github.rest.actions.getWorkflowRun({ ...repo, run_id: eventRun.id });
  if (run.status !== 'completed' || run.conclusion !== 'failure' || run.run_attempt !== eventRun.run_attempt) {
    core.info('No retry: the run is no longer the failed attempt that triggered this workflow.');
    return;
  }

  const jobs = await github.paginate(github.rest.actions.listJobsForWorkflowRun, {
    ...repo, run_id: run.id, filter: 'latest', per_page: 100,
  });
  const unsuccessful = jobs.filter(job => !['success', 'skipped'].includes(job.conclusion));
  if (!unsuccessful.length) return;
  for (const job of unsuccessful) {
    const checkId = Number(job.check_run_url?.match(/\/check-runs\/(\d+)$/)?.[1]);
    if (!checkId || (job.steps ?? []).length !== 0) {
      core.info(`No retry: ${job.name} was not an unstarted runner acquisition failure.`);
      return;
    }
    const annotations = await github.paginate(github.rest.checks.listAnnotations, {
      ...repo, check_run_id: checkId, per_page: 100,
    });
    if (!isAcquisitionFailure(job, annotations)) {
      core.info(`No retry: ${job.name} has no confirmed hosted runner acquisition failure.`);
      return;
    }
  }

  if (run.run_attempt >= MAX_ATTEMPTS) {
    core.setFailed(`Automatic retry limit reached for ${run.html_url}; investigate the persistent failure.`);
    return;
  }

  // Brief cooldown before asking GitHub to provision a runner again.
  await delay(60_000);
  // A person may have restarted or cancelled the run during the cooldown.
  const { data: current } = await github.rest.actions.getWorkflowRun({ ...repo, run_id: run.id });
  if (current.status !== 'completed' || current.conclusion !== 'failure' || current.run_attempt !== run.run_attempt) return;
  await github.rest.actions.reRunWorkflowFailedJobs({ ...repo, run_id: run.id });
  await core.summary.addHeading('Retrying release infrastructure failure')
    .addRaw(`Requested attempt ${run.run_attempt + 1} of ${MAX_ATTEMPTS} for ${run.html_url}. Successful jobs are preserved.`)
    .write();
}

module.exports = { isAcquisitionFailure, retryRelease };
