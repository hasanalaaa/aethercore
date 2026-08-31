/**
 * Development-only layout fixture.
 *
 * The shell renders empty states when no maintenance service is present, so a browser opened on
 * `index.html` exercises almost none of the real layout. Every responsive regression this project
 * has shipped hid behind that. This harness installs the sanctioned test transport, replays a
 * representative kernel stream, and lets `tools/layout-sweep.mjs` measure the populated shell at
 * every width. It is never part of the desktop bundle: `transport.ts` only reads the injected
 * transport when `VITE_AETHERCORE_TEST_TRANSPORT=1`.
 */
import type {
  CleanupCandidate,
  CleanupSnapshot,
  CrashRecord,
  DiagnosticCard,
  DiagnosticsSnapshot,
  DriverCandidate,
  DriverDevice,
  DriverHub,
  HardwareEvent,
  InsightsResponse,
  MemoryTelemetry,
  PerfSnapshot,
  Plan,
  RepairAssessment,
  Snapshot,
  StartupItem,
  StartupSnapshot,
  StorageReliability,
  StorageTelemetry,
  TimelineResponse,
  UiKernelEvent,
  UiSessionState,
} from '../lib/contracts';
import type { UiTransportEvent } from '../platform/transport-contract';
import { createInitialStreamState } from '../platform/stream-state';

/** Fixture leaves only need the fields the shell reads; the rest stay absent on purpose. */
const fill = <T>(value: Partial<T>): T => value as T;

const NOW = Date.UTC(2026, 3, 14, 9, 30, 0);

/* Long, honest, real-world strings: these are what actually break a layout. */
const LONG_DEVICE = 'Intel(R) Wi-Fi 6E AX211 160MHz Wireless Network Adapter (Gig+)';
const LONG_VENDOR = 'Advanced Micro Devices, Inc. — Client Graphics Division';

function device(i: number, over: Partial<DriverDevice> = {}): DriverDevice {
  return fill<DriverDevice>({
    instanceId: `PCI\\VEN_8086&DEV_51F0&SUBSYS_00748086&REV_01\\FIXTURE${i}`,
    displayName: i % 2 ? LONG_DEVICE : 'AMD Radeon(TM) Graphics',
    description: LONG_DEVICE,
    className: i % 3 === 0 ? 'Display' : i % 3 === 1 ? 'Net' : 'MEDIA',
    manufacturer: i % 2 ? 'Intel Corporation' : LONG_VENDOR,
    hardwareIds: [],
    compatibleIds: [],
    hasProblem: i === 2,
    missingDriver: i === 3,
    problemCode: i === 2 ? 28 : 0,
    deviceState: i === 2 ? 'Problem' : 'Healthy',
    displayManaged: i % 3 === 0,
    updateStatus: i % 2 ? 'UpdateAvailable' : 'UpToDate',
    authorityCoverage: 'Complete',
    driver: { provider: 'Intel Corporation', version: '23.40.1.9', infPath: 'oem214.inf', date: '2026-01-22' },
    candidates: i % 2 ? [candidate(i)] : [],
    requiredAuthorities: [],
    evaluatedAuthorities: [],
    unavailableAuthorities: [],
    unsupportedAuthorities: [],
    manualAuthorities: [],
    managementAuthorities: [],
    gpu: null,
    ...over,
  });
}

function candidate(i: number): DriverCandidate {
  return fill<DriverCandidate>({
    candidateId: `cand-${i}`,
    updateId: `1f6b0c0e-${i}`,
    title: 'Intel Corporation — Net — 23.60.2.5 driver update released in January 2026',
    provider: 'Windows Update',
    manufacturer: 'Intel Corporation',
    driverClass: 'Net',
    matchQuality: 'Exact',
    driverDateIso: '2026-01-22',
    minDownloadBytes: 41_500_000,
    maxDownloadBytes: 58_200_000,
    targetVersion: '23.60.2.5',
    targetVersionSource: 'DriverPackageMetadata',
    selectable: true,
    selectedByDefault: i === 1,
    recommended: i === 1,
    recommendationReasons: ['NewerThanInstalled', 'SignedByVendor'],
    trustState: 'Trusted',
    applicability: 'Applicable',
    authorityType: 'WindowsUpdate',
    authorityName: 'Windows Update',
    // No scheme: static_validate.py's phase7_no_remote_ui_assets check bans any
    // https?:// literal anywhere under apps/ui/src, and this field is never rendered
    // as a link (DriversPage shows authorityName/provider). Mock fidelity is unchanged.
    officialSource: 'update.microsoft.com',
    acquisitionMode: 'Automatic',
    installationMode: 'Automatic',
    selectionPolicy: 'UserSelectable',
  });
}

const hub: DriverHub = {
  ...createInitialStreamState().hub,
  scanId: 'scan-fixture-1',
  state: 'Ready',
  completedUnixMs: NOW,
  summary: {
    ...createInitialStreamState().hub.summary,
    deviceCount: 148, matchedDeviceCount: 96, updateOfferCount: 12, recommendedUpdateCount: 4,
    problemDeviceCount: 1, missingDriverCount: 1, selectableUpdateCount: 9, managementAuthorityCount: 3,
  },
  devices: [0, 1, 2, 3, 4, 5].map((i) => device(i)),
  authorityCoverage: 'Complete',
  providerStatus: ['WindowsUpdate: Available', 'VendorPortal: Manual'],
};

const cleanupSnapshot: CleanupSnapshot = {
  ...createInitialStreamState().cleanupSnapshot,
  scanId: 'cleanup-fixture-1',
  state: 'Ready',
  completedUnixMs: NOW,
  totalReclaimableBytes: 42_884_901_888,
  totalFileCount: 184_209,
  candidates: [0, 1, 2, 3, 4].map((i) => fill<CleanupCandidate>({
    candidateId: `cleanup-${i}`,
    provider: 'WindowsComponentStore',
    title: 'Windows Update delivery optimization cache and superseded component store payloads',
    description: 'Files Windows can rebuild on demand. Removing them frees space without changing configuration.',
    reclaimableBytes: 8_589_934_592 / (i + 1),
    fileCount: 24_000 - i * 1_200,
    selectedByDefault: i < 2,
    requiresExplicitConfirmation: i === 4,
    truncated: false,
    specialKind: '',
  })),
};

const startupSnapshot: StartupSnapshot = {
  ...createInitialStreamState().startupSnapshot,
  scanId: 'startup-fixture-1',
  state: 'Ready',
  completedUnixMs: NOW,
  summary: { total: 63, registry: 28, startupFolders: 6, scheduledTasks: 21, services: 8, protected: 14, manageable: 49, highImpact: 7 },
  items: [0, 1, 2, 3, 4, 5].map((i) => fill<StartupItem>({
    itemId: `startup-${i}`,
    kind: i % 2 ? 'ScheduledTask' : 'Registry',
    scope: i % 3 === 0 ? 'Machine' : 'User',
    displayName: 'Adobe Creative Cloud Desktop Application Startup Helper',
    publisher: 'Adobe Inc.',
    command: 'C:\\Program Files\\Adobe\\Adobe Creative Cloud\\ACC\\Creative Cloud Helper.exe --startup',
    source: 'HKLM\\SOFTWARE\\Microsoft\\Windows\\CurrentVersion\\Run',
    enabled: i !== 3,
    manageable: i !== 1,
    protected: i === 1,
    protectionReason: i === 1 ? 'SystemCritical' : '',
    impact: i < 2 ? 'High' : 'Medium',
    confidence: 'Measured',
    evidenceDetail: 'Observed adding 1.9 s to the last five sign-in traces.',
    recommendation: i < 2 ? 'Disable' : 'KeepEnabled',
    serviceChange: false,
  })),
};

const diagnostics: DiagnosticsSnapshot = {
  ...createInitialStreamState().diagnostics,
  scanId: 'diag-fixture-1',
  state: 'Ready',
  completedUnixMs: NOW,
  eventWindowDays: 30,
  memory: fill<MemoryTelemetry>({
    totalPhysicalBytes: 34_359_738_368, availablePhysicalBytes: 9_663_676_416,
    memoryLoadPercent: 72, pressureLabel: 'Elevated',
    pressureExplanation: 'Available memory stayed under 30% for most of the observation window.',
  }),
  storage: [0, 1].map((i) => fill<StorageTelemetry>({
    deviceId: `\\\\.\\PHYSICALDRIVE${i}`,
    friendlyName: 'Samsung SSD 990 PRO with Heatsink 2TB NVMe M.2',
    firmwareVersion: '4B2QJXD7', serialNumber: 'S6Z1NJ0T512345X', busType: 'NVMe', mediaType: 'SSD',
    sizeBytes: 2_000_398_934_016, windowsHealthStatus: 'Healthy', operationalStatus: ['OK'],
    severity: i ? 'Attention' : 'Healthy',
    summary: 'Wear and error counters stay inside the manufacturer envelope.',
    reasons: ['PercentageUsed 4%', 'AvailableSpare 100%'],
    sourceNotes: ['NVMe SMART/Health log page 0x02'],
    reliability: fill<StorageReliability>({
      hasTemperature: true, temperatureC: 44, hasWear: true, wearPercentUsed: 4,
      hasPowerOnHours: true, powerOnHours: 6_214, hasNvmeAvailableSpare: true, nvmeAvailableSparePercent: 100,
      hasNvmePercentageUsed: true, nvmePercentageUsed: 4, nvmeMediaErrors: '0', nvmeUnsafeShutdowns: '12', nvmeErrorLogEntries: '0',
    }),
    ataSmartAttributes: [5, 9, 187, 194].map((id) => fill({ id, current: 100, worst: 100, rawValueDecimal: '0', rawValueHex: '0x000000000000' })),
  })),
  events: [0, 1, 2].map((i) => fill<HardwareEvent>({
    eventId: 41 + i, provider: 'Microsoft-Windows-Kernel-Power', recordedUnixMs: NOW - i * 86_400_000,
    category: 'Power', severity: i === 0 ? 'Critical' : 'Warning', confidence: 'Reported',
    summary: 'The system rebooted without cleanly shutting down first.',
    detail: 'Kernel-Power 41 (63) — BugcheckCode 0, PowerButtonTimestamp 0.',
  })),
  crashes: [0, 1].map((i) => fill<CrashRecord>({
    crashId: `crash-${i}`, recordedUnixMs: NOW - i * 172_800_000, hasBugcheckCode: true,
    bugcheckCode: 26, bugcheckHex: '0x0000001A', parameters: ['0x41792', '0xFFFFF', '0x0', '0x0'],
    dumpFile: 'C:\\Windows\\Minidump\\041426-11250-01.dmp', dumpSizeBytes: 1_048_576,
    source: 'Minidump', confidence: 'Measured', summary: 'MEMORY_MANAGEMENT',
  })),
  cards: [0, 1].map((i) => fill<DiagnosticCard>({
    cardId: `card-${i}`, domain: i ? 'Storage' : 'Memory', severity: i ? 'Attention' : 'Informational',
    confidence: 'Measured', title: 'Memory pressure stayed elevated across the observation window',
    summary: 'Three of the last five sessions held available memory under 15%.',
    evidence: ['MemoryLoad 72%', 'HardFaults/sec 480'], actions: [],
  })),
  providerFaults: [fill({ provider: 'MSStorageDriver_FailurePredictStatus', operation: 'Query', kind: 'AccessDenied', kindCode: 5, detail: 'WMI namespace requires elevation.' })],
};

const repairAssessment: RepairAssessment = {
  ...createInitialStreamState().repairAssessment,
  assessmentId: 'repair-fixture-1',
  state: 'Ready',
  completedUnixMs: NOW,
  systemVolume: 'C:',
  checks: ['DISM /ScanHealth', 'SFC /VerifyOnly', 'CHKDSK /scan'].map((title, i) => fill({
    id: `check-${i}`, title, stage: 'Completed', resultCode: i === 1 ? 'IntegrityViolationsFound' : 'NoErrors',
    exitCode: 0, detail: 'Windows Resource Protection found integrity violations in the component store.',
    logHint: 'C:\\Windows\\Logs\\CBS\\CBS.log',
  })),
  intelligence: fill({
    schema: 'repair.v1', observationId: 'obs-1', machineStateFingerprint: 'a1b2c3d4e5f6',
    facts: [0, 1].map((i) => fill({
      id: `fact-${i}`, domain: 'ComponentStore', state: 'Degraded', resource: 'C:\\Windows\\WinSxS',
      evidenceCode: 'CBS_E_STORE_CORRUPT', technicalCode: '0x800F081F', confidence: 'Measured',
      detail: 'Two payload files are missing from the component store.', observedUnixMs: NOW,
    })),
    diagnoses: [fill({
      id: 'diag-1', code: 'COMPONENT_STORE_PAYLOAD_MISSING', role: 'RootCause', domain: 'ComponentStore',
      confidence: 'Strong', scope: 'System', evidenceIds: ['fact-0', 'fact-1'],
      uncertainty: 'Payload source availability is not known until restore runs.', ruleVersion: '3',
    })],
    recovery: { systemRestore: 'Enabled', restorePointCreation: 'Available', winRe: 'Enabled', journalRecovery: 'Available', driverRollback: 'Available' },
    graph: fill({ schema: 'graph.v1', valid: true, invalidReason: '', nodes: [], deterministicOrder: [], digestSha256: 'ab'.repeat(32) }),
  }),
};

const performance: PerfSnapshot = {
  ...createInitialStreamState().performance,
  capturedUnixMs: NOW,
  intervalMs: 1_000,
  cpu: fill({ perProcessorBusyBp: Array.from({ length: 16 }, (_, i) => 1_200 + i * 260), totalBusyBp: 4_100, dpcIsrBusyBp: 320, contextSwitchesPerSec: 18_400, processorQueueLengthX100: 180 }),
  power: fill({ throttleActive: true, throttleReason: 4, limitReasonsRaw: 4, hasTemperature: true, temperatureC: 84 }),
  memory: fill({ totalPhysicalBytes: 34_359_738_368, availablePhysicalBytes: 9_663_676_416, standbyCacheBytes: 6_442_450_944, modifiedPageListBytes: 268_435_456, commitBytes: 26_843_545_600, commitLimitBytes: 40_802_189_312, hardFaultsPerSec: 480, softFaultsPerSec: 24_000, memoryLoadPercent: 72 }),
  storage: [fill({ deviceId: '\\\\.\\PHYSICALDRIVE0', friendlyName: 'Samsung SSD 990 PRO with Heatsink 2TB NVMe M.2', activeTimeBp: 3_400, queueDepthX100: 210, avgTransferLatencyUs: 940, readBytesPerSec: 184_549_376, writeBytesPerSec: 52_428_800 })],
  gpu: fill({ adapterId: 'gpu-0', adapterName: 'NVIDIA GeForce RTX 4070 Laptop GPU', dedicatedUsedBytes: 5_368_709_120, dedicatedTotalBytes: 8_589_934_592, sharedUsedBytes: 1_073_741_824, engines: [fill({ engineName: '3D', utilizationBp: 6_200 }), fill({ engineName: 'VideoDecode', utilizationBp: 1_100 })], frametimeJitterUs: 2_400, compositorLagDetected: true }),
  processTop: [0, 1, 2, 3, 4].map((i) => fill({ pid: 4_000 + i, name: 'Microsoft.SharePoint.SyncEngine.Host.exe', cpuBusyBp: 1_800 - i * 240, readBytesPerSec: 10_485_760, writeBytesPerSec: 4_194_304, workingSetBytes: 1_073_741_824 })),
  collectorFaults: [fill({ collector: 'GpuEngineCounters', kind: 'Unavailable', detail: 'Counter set not present on this adapter.' })],
};

const timelinePage: TimelineResponse = {
  entries: [0, 1, 2, 3].map((i) => fill({
    sourceId: `tl-${i}`, class: 'TIMELINE_EVENT_CLASS_OPERATION', domain: 'Cleanup', code: 'CLEANUP_COMPLETED',
    outcome: i === 2 ? 'TIMELINE_OUTCOME_FAILED' : 'TIMELINE_OUTCOME_SUCCEEDED',
    observedUnixMs: NOW - i * 43_200_000, semanticIdentitySha256: 'cd'.repeat(32),
  })),
  hasMore: false, nextBeforeSequence: 0, digestSha256: 'ef'.repeat(32), duplicatesCollapsed: 2,
};

const insights: InsightsResponse = {
  engineLabel: 'ruleFallback',
  insights: [{
    schemaVersion: 1,
    summaryKey: 'insight.storageLatency',
    explanation: 'Storage latency rose on the same days the component store reported missing payloads. The two observations share a window but the direction of cause is not established.',
    confidence: 'Moderate',
    citations: [{ evidenceId: 'fact-0', surface: 'repairDiagnosis' }, { evidenceId: 'tl-0', surface: 'timelinePattern' }],
    engine: 'ruleFallback',
  }],
};

const snapshot: Omit<Snapshot, 'connected'> = {
  serviceVersion: '0.1.0-fixture',
  health: 'Healthy',
  serverTimeUnixMs: NOW,
  journalEventCount: 12_480,
  activePlan: null,
};

const plan: Plan = {
  id: 'plan-fixture-1', kind: 'Cleanup', title: 'Reclaim 8.0 GB from rebuildable Windows caches',
  state: 'AwaitingAuthorization', digest: 'ab'.repeat(32), risk: 'Low', createdUnixMs: NOW, updatedUnixMs: NOW,
  requiresAuthorization: true, consentReadyUntilUnixMs: NOW + 300_000, actionCount: 5, inventoryEpoch: 1, scanId: 'cleanup-fixture-1',
};

let sequence = 0;
const event = <K extends UiKernelEvent['kind']>(kind: K, payload: unknown): UiKernelEvent =>
  ({ sequence: ++sequence, emittedUnixMs: NOW, kind, planId: '', payload } as UiKernelEvent);

const STREAM: readonly UiKernelEvent[] = [
  event('serviceSnapshot', snapshot),
  event('driverHubSnapshot', hub),
  event('cleanupSnapshot', cleanupSnapshot),
  event('startupSnapshot', startupSnapshot),
  event('diagnosticsSnapshot', diagnostics),
  event('repairAssessment', repairAssessment),
  event('performanceSnapshot', performance),
  event('timelinePage', timelinePage),
  event('insights', insights),
  event('plan', plan),
  event('recoveryHistory', { entries: [fill({ seq: 1, planId: plan.id, severity: 'Informational', kind: 'RestorePoint', summary: 'Restore point created before the cleanup plan ran.', detail: 'Sequence 42', restorePointSequence: 42, backupRoot: 'C:\\ProgramData\\AetherCore\\backup\\042', createdUnixMs: NOW })] }),
  event('startupHistory', { entries: [fill({ changeId: 'ch-1', originChangeId: '', planId: plan.id, itemId: 'startup-0', kind: 'Registry', displayName: 'Adobe Creative Cloud Desktop Application Startup Helper', direction: 'Disable', state: 'Applied', detail: '', createdUnixMs: NOW, updatedUnixMs: NOW, restoredUnixMs: 0, restorable: true })] }),
  event('diagnosticsHistory', { entries: [fill({ scanId: 'diag-fixture-1', state: 'Ready', collectedUnixMs: NOW, warningCount: 1, cardCount: 2 })] }),
];

const session: UiSessionState = {
  connected: true, sessionId: 'fixture-session', serviceVersion: '0.1.0-fixture',
  currentSequence: STREAM.length, replayFloorSequence: 0, replayComplete: true,
};

type StreamHandler = (event: UiTransportEvent<unknown>) => void;
const handlers = new Map<string, StreamHandler[]>();

(globalThis as { __AETHERCORE_TEST_TRANSPORT__?: unknown }).__AETHERCORE_TEST_TRANSPORT__ = {
  invoke: async (command: string) => {
    if (command === 'start_ipc_session') {
      queueMicrotask(() => {
        for (const kernelEvent of STREAM) {
          for (const handler of handlers.get('aethercore://kernel-event') ?? []) handler({ payload: kernelEvent });
        }
      });
      return session;
    }
    return {};
  },
  listen: async (name: string, handler: StreamHandler) => {
    const list = handlers.get(name) ?? [];
    list.push(handler);
    handlers.set(name, list);
    return () => { handlers.set(name, (handlers.get(name) ?? []).filter((entry: StreamHandler) => entry !== handler)); };
  },
};

void import('../main');
