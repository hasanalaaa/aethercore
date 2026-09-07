/**
 * The Overview's instrument: what the four ported sections are allowed to say.
 *
 * The shell this composition comes from is a mockup and its numbers are drawn,
 * not measured — `94` health index, `96` responsiveness, `11.4 s` boot, `214
 * signals`. None of them survive here. Every function below either names the
 * IPC field it reads or returns `undefined`, and `undefined` renders as an em
 * dash, never as a zero.
 *
 * The distinction P42→P45 spent four phases building at the type level is the
 * one this file exists to preserve at the view layer: a channel whose provider
 * reported `0` is a MEASURED ZERO and prints `0`; a channel whose provider was
 * never asked is UNMEASURED and prints `—`. That is why every reading here is
 * gated on the presence of its provider object (`perf.cpu !== null`,
 * `power.hasTemperature`) and never on the truthiness of the number.
 */
import type { PerfSnapshot, Snapshot } from '../../lib/contracts';
import type { StreamState } from '../../platform/stream-state';
import type { Evidence } from '../../design/signature';
import type { PageId } from '../../lib/navigation';
import { formatNumber, t, tp, type Locale } from '../../lib/i18n';
import { formatBytes } from '../shared';

/** A percentage rendered the way the locale writes one. */
function percent(value: number, locale: Locale): string {
  return formatNumber(value / 100, locale, { style: 'percent', maximumFractionDigits: 0 });
}

function count(value: number, locale: Locale): string {
  return formatNumber(value, locale);
}

/**
 * A temperature with its unit attached by the locale rather than by us.
 * `${n} °C` reverses to `C° ٨٤` under RTL; `style: 'unit'` orders it correctly
 * in both directions.
 */
function celsius(value: number, locale: Locale): string {
  return formatNumber(value, locale, { style: 'unit', unit: 'celsius', unitDisplay: 'narrow' });
}

// ---------------------------------------------------------------------------
// §A — the health orb and its four channel rails
// ---------------------------------------------------------------------------

export type HealthChannel = {
  id: 'cpu' | 'memory' | 'disk' | 'thermal';
  label: string;
  /**
   * 0–100 against the channel's own scale, for the rail's bar.
   *
   * Absent means one of two different things, and the rail says which: the
   * channel has no reading, or the reading has no scale to normalize against.
   * Thermals is the second case — mapping °C onto 0–100 needs a ceiling this
   * product does not measure, so the bar stays empty and the value still shows.
   */
  pct?: number;
  /** The reading. Absent means not collected; it is never rendered as 0. */
  value?: string;
  /** Whether the channel is one of the utilization channels headroom averages. */
  counted: boolean;
  /** One line of the orb's raw observation block. */
  raw: string;
  /** A state the SERVICE asserted, not one this file inferred. */
  serviceState?: string;
};

/** The busiest storage device in the sample, or null if none was reported. */
function busiestDevice(perf: PerfSnapshot) {
  if (!perf.storage.length) return null;
  return perf.storage.reduce((worst, device) => (device.activeTimeBp > worst.activeTimeBp ? device : worst));
}

export function healthChannels(perf: PerfSnapshot, locale: Locale): HealthChannel[] {
  const unmeasured = t('overview.channelUnmeasured', locale);
  const device = busiestDevice(perf);
  const cpuPct = perf.cpu ? perf.cpu.totalBusyBp / 100 : undefined;
  const memoryPct = perf.memory ? perf.memory.memoryLoadPercent : undefined;
  const diskPct = device ? device.activeTimeBp / 100 : undefined;
  const thermal = perf.power?.hasTemperature ? perf.power.temperatureC : undefined;

  return [
    {
      id: 'cpu',
      label: t('overview.channelCpu', locale),
      pct: cpuPct,
      value: cpuPct === undefined ? undefined : percent(cpuPct, locale),
      counted: true,
      raw: perf.cpu
        ? `cpu.totalBusyBp = ${perf.cpu.totalBusyBp} bp -> ${(perf.cpu.totalBusyBp / 100).toFixed(1)}%  (PerfSnapshot.cpu)`
        : `cpu = null  (${unmeasured})`,
    },
    {
      id: 'memory',
      label: t('overview.channelMemory', locale),
      pct: memoryPct,
      value: perf.memory
        ? formatBytes(perf.memory.totalPhysicalBytes - perf.memory.availablePhysicalBytes, locale)
        : undefined,
      counted: true,
      raw: perf.memory
        ? `memory.memoryLoadPercent = ${perf.memory.memoryLoadPercent}%  in use = total ${perf.memory.totalPhysicalBytes} - available ${perf.memory.availablePhysicalBytes} B  (PerfSnapshot.memory)`
        : `memory = null  (${unmeasured})`,
    },
    {
      id: 'disk',
      label: t('overview.channelDisk', locale),
      pct: diskPct,
      value: diskPct === undefined ? undefined : percent(diskPct, locale),
      counted: true,
      raw: device
        ? `storage.activeTimeBp = ${device.activeTimeBp} bp -> ${(device.activeTimeBp / 100).toFixed(1)}%  on ${device.deviceId}  (PerfSnapshot.storage, busiest of ${perf.storage.length})`
        : `storage = []  (${unmeasured})`,
    },
    {
      id: 'thermal',
      label: t('overview.channelThermal', locale),
      // No pct on purpose. See the type.
      value: thermal === undefined ? undefined : celsius(thermal, locale),
      counted: false,
      raw: perf.power?.hasTemperature
        ? `power.temperatureC = ${perf.power.temperatureC}  hasTemperature = true  throttleActive = ${perf.power.throttleActive}  (PerfSnapshot.power)`
        : `power.hasTemperature = false  (${unmeasured})`,
      serviceState: perf.power?.throttleActive ? t('overview.throttling', locale) : undefined,
    },
  ];
}

/**
 * Headroom: `100 − mean(utilization)` over the channels that both have a
 * reading and normalize to 0–100 without an invented ceiling.
 *
 * Every input is a percentage the service reported, so no constant enters the
 * arithmetic. Returns `undefined` when nothing has been measured — the orb then
 * reads `—` and `NO BASELINE`, which is what the shell's own unscanned variant
 * does.
 *
 * It is rendered with a percent sign rather than as the shell's bare index, for
 * two reasons: it IS percentage points, and `verify-numbers.mjs` only examines
 * numbers of that shape. A bare `51` would have been the single most prominent
 * figure on the screen and invisible to the gate that exists to trace it.
 */
export function headroomLabel(value: number | undefined, locale: Locale): string {
  return value === undefined ? '—' : percent(value, locale);
}

export function headroom(channels: readonly HealthChannel[]): number | undefined {
  const used = channels.filter((channel) => channel.counted && channel.pct !== undefined);
  if (!used.length) return undefined;
  const mean = used.reduce((total, channel) => total + (channel.pct as number), 0) / used.length;
  return Math.round(100 - mean);
}

/** The orb's citation. The arithmetic is printed so it can be redone by hand. */
export function headroomEvidence(
  channels: readonly HealthChannel[],
  perf: PerfSnapshot,
  locale: Locale,
): Evidence | null {
  const used = channels.filter((channel) => channel.counted && channel.pct !== undefined);
  if (!used.length) return null;
  const terms = used.map((channel) => (channel.pct as number).toFixed(1)).join(' + ');
  const mean = used.reduce((total, channel) => total + (channel.pct as number), 0) / used.length;
  return {
    cite: tp('unit.channel', locale, used.length),
    raw: [
      t('overview.orbEvidenceHead', locale),
      `headroom = 100 - (${terms}) / ${used.length} = ${Math.round(100 - mean)}`,
      '',
      ...channels.map((channel) => `${channel.label}  ${channel.raw}`),
      '',
      `captured ${perf.capturedUnixMs} unix-ms, interval ${perf.intervalMs} ms`,
      t('overview.orbExcludedThermal', locale),
      t('overview.orbExcludedDisk', locale),
    ].join('\n'),
  };
}

// ---------------------------------------------------------------------------
// §B — action items
// ---------------------------------------------------------------------------

export type ActionItem = {
  id: string;
  title: string;
  meta: string;
  value: string;
  tag: string;
  tone: 'critical' | 'attention' | 'info' | 'healthy';
  page: PageId;
  evidence: Evidence;
};

/**
 * Every row is a summary of a scan that has actually reported, and carries that
 * scan's own identity as its citation. A source that has not run contributes no
 * row at all — a screen that has measured nothing shows the empty state rather
 * than a list of reassuring zeroes.
 */
export function actionItems(state: StreamState, locale: Locale): ActionItem[] {
  const items: ActionItem[] = [];
  const { diagnostics, deepScan, hub, cleanupSnapshot } = state;

  if (diagnostics.state === 'Ready' && diagnostics.scanId && diagnostics.crashes.length) {
    const newest = diagnostics.crashes[0];
    items.push({
      id: 'crashes',
      title: t('overview.itemCrashes', locale),
      meta: `${newest.bugcheckHex} · ${newest.source}`,
      value: tp('unit.crashRecord', locale, diagnostics.crashes.length),
      tag: t('severity.ActionRequired', locale),
      tone: 'critical',
      page: 'crash',
      evidence: {
        cite: `${diagnostics.scanId} · ${tp('unit.crashRecord', locale, diagnostics.crashes.length)}`,
        raw: diagnostics.crashes
          .map((crash) => `${crash.bugcheckHex}  ${crash.summary}  source=${crash.source}  confidence=${crash.confidence}  recorded=${crash.recordedUnixMs}`)
          .join('\n'),
      },
    });
  }

  const findingTotal = deepScan.summary
    ? deepScan.summary.critical + deepScan.summary.high + deepScan.summary.moderate + deepScan.summary.low
    : 0;
  if (deepScan.scanId && deepScan.completedUnixMs > 0 && deepScan.summary && findingTotal > 0) {
    const summary = deepScan.summary;
    items.push({
      id: 'findings',
      title: t('overview.itemFindings', locale),
      meta: `${t('severity.ActionRequired', locale)} ${count(summary.critical, locale)} · ${t('severity.Attention', locale)} ${count(summary.high, locale)}`,
      value: tp('unit.finding', locale, findingTotal),
      tag: summary.recommendedActions > 0 ? tp('unit.action', locale, summary.recommendedActions) : t('severity.Info', locale),
      tone: summary.critical > 0 ? 'critical' : summary.high > 0 ? 'attention' : 'info',
      page: 'deepScan',
      evidence: {
        cite: `${deepScan.scanId} · ${count(deepScan.factsCount, locale)}`,
        raw: [
          `scan ${deepScan.scanId}  completed=${deepScan.completedUnixMs}  facts=${deepScan.factsCount}`,
          `critical=${summary.critical} high=${summary.high} moderate=${summary.moderate} low=${summary.low} informational=${summary.informational}`,
          `recommendedActions=${summary.recommendedActions} healthyChecks=${summary.healthyChecks}`,
          `ruleEngineVersion=${deepScan.ruleEngineVersion}  fingerprint=${deepScan.machineStateFingerprint}`,
        ].join('\n'),
      },
    });
  }

  if (hub.state === 'Ready' && hub.scanId) {
    const problems = hub.summary.problemDeviceCount + hub.summary.missingDriverCount;
    if (problems > 0) {
      items.push({
        id: 'driver-problems',
        title: t('overview.itemDriverProblems', locale),
        meta: t('overview.itemDriverProblemsMeta', locale, {
          problem: count(hub.summary.problemDeviceCount, locale),
          missing: count(hub.summary.missingDriverCount, locale),
        }),
        value: tp('unit.device', locale, problems),
        tag: t('severity.Attention', locale),
        tone: 'attention',
        page: 'drivers',
        evidence: {
          cite: `${hub.scanId} · ${tp('unit.device', locale, hub.summary.deviceCount)}`,
          raw: [
            `scan ${hub.scanId}  completed=${hub.completedUnixMs}  epoch=${hub.inventoryEpoch}`,
            `deviceCount=${hub.summary.deviceCount} problemDeviceCount=${hub.summary.problemDeviceCount} missingDriverCount=${hub.summary.missingDriverCount}`,
            `authorityCoverage=${hub.authorityCoverage}`,
          ].join('\n'),
        },
      });
    }
    if (hub.summary.selectableUpdateCount > 0) {
      items.push({
        id: 'driver-updates',
        title: t('overview.itemDriverUpdates', locale),
        meta: t('overview.itemDriverUpdatesMeta', locale, {
          offers: count(hub.summary.updateOfferCount, locale),
          devices: count(hub.summary.matchedDeviceCount, locale),
        }),
        value: tp('unit.update', locale, hub.summary.selectableUpdateCount),
        tag: t('severity.Info', locale),
        tone: 'info',
        page: 'drivers',
        evidence: {
          cite: `${hub.scanId} · ${tp('unit.offer', locale, hub.summary.updateOfferCount)}`,
          raw: [
            `scan ${hub.scanId}  completed=${hub.completedUnixMs}`,
            `updateOfferCount=${hub.summary.updateOfferCount} selectableUpdateCount=${hub.summary.selectableUpdateCount} recommendedUpdateCount=${hub.summary.recommendedUpdateCount}`,
            `matchedDeviceCount=${hub.summary.matchedDeviceCount} managementAuthorityCount=${hub.summary.managementAuthorityCount}`,
            ...hub.providerStatus,
          ].join('\n'),
        },
      });
    }
  }

  if (diagnostics.state === 'Ready' && diagnostics.scanId && diagnostics.events.length) {
    items.push({
      id: 'events',
      title: t('overview.itemEvents', locale, { window: tp('unit.day', locale, diagnostics.eventWindowDays) }),
      meta: diagnostics.events[0].provider,
      value: tp('unit.event', locale, diagnostics.events.length),
      tag: t('severity.Attention', locale),
      tone: 'attention',
      page: 'hardware',
      evidence: {
        cite: `${diagnostics.scanId} · ${tp('unit.day', locale, diagnostics.eventWindowDays)}`,
        raw: diagnostics.events
          .map((entry) => `${entry.provider} ${entry.eventId}  severity=${entry.severity}  confidence=${entry.confidence}  recorded=${entry.recordedUnixMs}`)
          .join('\n'),
      },
    });
  }

  if (cleanupSnapshot.state === 'Ready' && cleanupSnapshot.scanId && cleanupSnapshot.totalReclaimableBytes > 0) {
    items.push({
      id: 'reclaimable',
      title: t('overview.itemReclaimable', locale),
      meta: tp('unit.category', locale, cleanupSnapshot.candidates.length),
      value: formatBytes(cleanupSnapshot.totalReclaimableBytes, locale),
      tag: t('severity.Normal', locale),
      tone: 'healthy',
      page: 'cleanup',
      evidence: {
        cite: `${cleanupSnapshot.scanId} · ${tp('unit.file', locale, cleanupSnapshot.totalFileCount)}`,
        raw: [
          `scan ${cleanupSnapshot.scanId}  completed=${cleanupSnapshot.completedUnixMs}  epoch=${cleanupSnapshot.inventoryEpoch}`,
          `totalReclaimableBytes=${cleanupSnapshot.totalReclaimableBytes}  totalFileCount=${cleanupSnapshot.totalFileCount}`,
          ...cleanupSnapshot.candidates.map((candidate) => `${candidate.provider}  ${candidate.reclaimableBytes} B  ${candidate.fileCount} files`),
        ].join('\n'),
      },
    });
  }

  return items;
}

// ---------------------------------------------------------------------------
// §C — diagnostic telemetry tiles
// ---------------------------------------------------------------------------

export type TelemetryTile = {
  id: string;
  label: string;
  /** Absent means not collected. A provider that reported 0 gives "0". */
  value?: string;
  unit: string;
  /**
   * Provenance, not a delta. The shell put a change-over-time here ("+4 vs
   * baseline"); nothing in this product retains a series, so the line names
   * where the reading came from instead.
   */
  note: string;
};

export function telemetryTiles(perf: PerfSnapshot, snapshot: Snapshot, locale: Locale): TelemetryTile[] {
  const device = busiestDevice(perf);
  /**
   * `PerfSnapshot.intervalMs` is the window the counters were observed over —
   * what `platform.sample(interval)` was given — not a repeat rate. It read
   * "sampled every 1000 ms", which asserted a cadence: true of the Performance
   * screen's sampler, and false of this screen, which reads every 5 s (§51.1).
   * The window is what the field actually is, so that is what it now says.
   */
  const sampled = t('overview.noteSampling', locale, { interval: count(perf.intervalMs, locale) });
  return [
    {
      id: 'disk-latency',
      label: t('overview.tileDiskLatency', locale),
      value: device ? formatNumber(device.avgTransferLatencyUs / 1000, locale, { minimumFractionDigits: 2, maximumFractionDigits: 2 }) : undefined,
      unit: t('overview.unitMs', locale),
      note: device ? device.friendlyName : t('overview.channelUnmeasured', locale),
    },
    {
      id: 'hard-faults',
      label: t('overview.tileHardFaults', locale),
      value: perf.memory ? count(perf.memory.hardFaultsPerSec, locale) : undefined,
      unit: t('overview.unitPerSecond', locale),
      note: perf.memory ? sampled : t('overview.channelUnmeasured', locale),
    },
    {
      id: 'context-switches',
      label: t('overview.tileContextSwitches', locale),
      value: perf.cpu ? count(perf.cpu.contextSwitchesPerSec, locale) : undefined,
      unit: t('overview.unitPerSecond', locale),
      note: perf.cpu ? sampled : t('overview.channelUnmeasured', locale),
    },
    {
      id: 'journal',
      label: t('overview.tileJournal', locale),
      value: snapshot.connected ? count(snapshot.journalEventCount, locale) : undefined,
      unit: '',
      note: snapshot.connected
        ? t('common.engineOnline', locale, { version: snapshot.serviceVersion })
        : t('common.engineOffline', locale),
    },
  ];
}
