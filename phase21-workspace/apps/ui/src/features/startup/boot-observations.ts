import type { BootMeasurement, BootDelay, StartupItem } from '../../lib/contracts';
const positive = (n: unknown): n is number => typeof n === 'number' && Number.isSafeInteger(n) && n > 0;
const u32 = (n: unknown): n is number => typeof n === 'number' && Number.isInteger(n) && n >= 0 && n <= 0xffffffff;
export function classifiedBoot(b: BootMeasurement): boolean {
  return b.completedMeasurement === true && b.hasRawClass === true && b.rawClassVersion === 1 && u32(b.rawClassValue)
    && b.hasSystemBootInstance === true && positive(b.systemBootInstance) && u32(b.systemBootInstance)
    && b.hasDuration === true && positive(b.durationMs) && u32(b.durationMs) && positive(b.recordedUnixMs)
    && b.hasBootStart === true && b.hasBootEnd === true && positive(b.bootStartUnixMs) && positive(b.bootEndUnixMs)
    && b.bootStartUnixMs < b.bootEndUnixMs && b.bootEndUnixMs <= b.recordedUnixMs;
}
export function bootComparison(b: BootMeasurement) {
  const c = b.comparison;
  if (!classifiedBoot(b) || !c || c.sampleCount !== 3 || !positive(c.medianMs) || !u32(c.medianMs)
      || !positive(c.windowStartUnixMs) || c.windowStartUnixMs >= c.windowEndUnixMs || c.windowEndUnixMs !== b.recordedUnixMs) return null;
  const baseline = c.hasBaseline === true && c.baselineCount === 5 && positive(c.baselineMs) && u32(c.baselineMs)
    && positive(c.baselineWindowStartUnixMs) && c.baselineWindowStartUnixMs < c.baselineWindowEndUnixMs
    && c.baselineWindowEndUnixMs < b.recordedUnixMs;
  return { ...c, hasBaseline: baseline };
}
// Lexical absolute identity only: no basename, display name, environment or filesystem probe.
function fullPath(value: string): string | null {
  if (value.length > 4096 || !/^[A-Za-z]:\\[^\x00-\x1f"%]+$/.test(value)) return null;
  const components = value.slice(3).split('\\');
  if (components.some(c => !c || c === '.' || c === '..' || /[:/]|[. ]$/.test(c))) return null;
  return value.toLowerCase();
}
function executable(item: StartupItem): string | null {
  const text = item.command.trim();
  const quoted = /^"([^"\r\n]+)"(?:\s|$)/.exec(text);
  if (quoted) return fullPath(quoted[1]);
  const first = /^([^\s"]+)(?:\s|$)/.exec(text)?.[1];
  // Unquoted paths with spaces cannot be reconstructed safely from a command line.
  return first && /\.(exe|com|bat|cmd)$/i.test(first) ? fullPath(first) : null;
}
function compatible(kind: string, eventId: number): boolean {
  return eventId === 103 ? kind === 'Service' : eventId === 101 && ['RegistryRun','RegistryRunOnce','StartupFolder','ScheduledTask'].includes(kind);
}
export function historicalDelay(item: StartupItem, inventory: readonly StartupItem[], boots: readonly BootMeasurement[]) {
  const path = executable(item);
  if (!path || inventory.filter(i => executable(i) === path).length !== 1) return null;
  for (const boot of boots) {
    if (!classifiedBoot(boot)) continue;
    const matches = (boot.delays ?? []).filter((d: BootDelay) => compatible(item.kind,d.eventId) && fullPath(d.fullPath) === path
      && positive(d.totalTimeMs) && u32(d.totalTimeMs) && u32(d.degradationTimeMs) && d.degradationTimeMs <= d.totalTimeMs
      && positive(d.recordedUnixMs) && d.recordedUnixMs >= boot.bootStartUnixMs!);
    if (matches.length === 1) return { boot, delay: matches[0] };
    if (matches.length > 1) return null;
  }
  return null;
}
