/** What `fleet_schedule_run_due` returns (apps/desktop/src/main.rs). */
export type RunDueResult = {
  ran: number;
  runs: { scheduleId: string; hostsAttempted: number; hostsOk: number; hostsFailed: number; error?: string | null }[];
  error?: string | null;
};

/**
 * Whether a run of the due schedules succeeded, and the hosts it counted. P75: the page reported
 * every run as processed, including one that could not read its schedules or whose hosts failed.
 */
export function runDueOutcome(result: RunDueResult): { ok: boolean; failed: number; attempted: number; detail: string } {
  const failed = result.runs.reduce((sum, run) => sum + run.hostsFailed, 0);
  const attempted = result.runs.reduce((sum, run) => sum + run.hostsAttempted, 0);
  const detail = [result.error, ...result.runs.map((run) => run.error)].filter((e): e is string => !!e).join('; ');
  return { ok: failed === 0 && detail === '', failed, attempted, detail };
}
