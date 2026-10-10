import type { ProcessCpuTopEntry } from '../../lib/contracts';

const LIMIT = 5;

/** The busiest sampled processes, by processor time then memory; an entry with no name is not shown. */
export function topConsumers(entries: readonly ProcessCpuTopEntry[], limit: number = LIMIT): ProcessCpuTopEntry[] {
  return entries
    .filter((entry) => entry.name.length > 0)
    .sort((a, b) => b.cpuBusyBp - a.cpuBusyBp || b.workingSetBytes - a.workingSetBytes)
    .slice(0, limit);
}
