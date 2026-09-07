import { get, writable } from 'svelte/store';
import type {
  BottleneckReport,
  PerfSnapshot,
  OptimizationStatus,
  CleanupSnapshot,
  CleanupStatus,
  DeepScanHistoryEntry,
  DeepScanSnapshot,
  DiagnosticHistoryEntry,
  DiagnosticsSnapshot,
  DriverHub,
  DriverInstallStatus,
  HubSummary,
  Plan,
  ProgressTelemetry,
  SchedulerEvent,
  UpdateSnapshot,
  SupportBundleEvent,
  TimelineResponse,
  CareRunStatus,
  InsightsResponse,
  RecoveryEntry,
  RepairAssessment,
  Snapshot,
  StartupHistoryEntry,
  StartupSnapshot,
  StartupStatus,
  SystemRepairStatus,
  UiKernelEvent,
  UiSessionState,
  UiStreamReset,
} from '../lib/contracts';

/**
 * How many kernel events the Overview's service log keeps.
 *
 * The service log is the one log on that screen with a real source: every event
 * the kernel pushes already carries its own sequence, emission time and kind.
 * Bounded because this is a live stream with no end, and the section shows a
 * window, not a history.
 */
export const SERVICE_LOG_LIMIT = 40;

export type StreamState = {
  snapshot: Snapshot;
  hub: DriverHub;
  installPlan: Plan | null;
  installStatus: DriverInstallStatus | null;
  recoveryEntries: RecoveryEntry[];
  repairAssessment: RepairAssessment;
  repairPlan: Plan | null;
  repairStatus: SystemRepairStatus | null;
  cleanupSnapshot: CleanupSnapshot;
  cleanupPlan: Plan | null;
  cleanupStatus: CleanupStatus | null;
  startupSnapshot: StartupSnapshot;
  startupPlan: Plan | null;
  startupStatus: StartupStatus | null;
  startupHistory: StartupHistoryEntry[];
  diagnostics: DiagnosticsSnapshot;
  deepScan: DeepScanSnapshot;
  deepScanHistory: DeepScanHistoryEntry[];
  diagnosticHistory: DiagnosticHistoryEntry[];
  schedulerEvent: SchedulerEvent | null;
  updateSnapshot: UpdateSnapshot;
  supportBundleEvent: SupportBundleEvent | null;
  performance: PerfSnapshot;
  performanceWindow: PerfSnapshot[];
  bottleneckReport: BottleneckReport | null;
  optimizationStatus: OptimizationStatus | null;
  perfSampling: boolean;
  timelinePage: TimelineResponse | null;
  careStatus: CareRunStatus | null;
  insights: InsightsResponse | null;
  session: UiSessionState;
  /** The last SERVICE_LOG_LIMIT kernel events, oldest first. */
  serviceLog: readonly UiKernelEvent[];
  lastKernelSequence: number;
};

export const emptyHubSummary: HubSummary = {
  deviceCount: 0,
  missingDriverCount: 0,
  problemDeviceCount: 0,
  updateOfferCount: 0,
  matchedDeviceCount: 0,
  matchedCandidateCount: 0,
  selectableUpdateCount: 0,
  vendorManagedGpuCount: 0,
  vendorManagedDisplayCount: 0,
  recommendedUpdateCount: 0,
  optionalUpdateCount: 0,
  vendorManagedUpdateCount: 0,
  statusUnknownCount: 0,
  managementAuthorityCount: 0,
};


export function createInitialStreamState(): StreamState {
  return {
    snapshot: {
      connected: false,
      serviceVersion: '—',
      health: 'Connecting',
      serverTimeUnixMs: 0,
      journalEventCount: 0,
      activePlan: null,
    },
    hub: {
      scanId: '',
      state: 'Idle',
      inventoryEpoch: 0,
      startedUnixMs: 0,
      completedUnixMs: 0,
      errorMessage: '',
      summary: { ...emptyHubSummary },
      devices: [],
      unmatchedOffers: [],
      warnings: [],
      authorityCoverage: 'Unknown',
      providerStatus: [],
    },
    installPlan: null,
    installStatus: null,
    recoveryEntries: [],
    repairAssessment: {
      assessmentId: '',
      state: 'Idle',
      startedUnixMs: 0,
      completedUnixMs: 0,
      errorMessage: '',
      systemVolume: '',
      checks: [],
      intelligence: null,
    },
    repairPlan: null,
    repairStatus: null,
    cleanupSnapshot: {
      scanId: '',
      state: 'Idle',
      inventoryEpoch: 0,
      startedUnixMs: 0,
      completedUnixMs: 0,
      errorMessage: '',
      totalReclaimableBytes: 0,
      totalFileCount: 0,
      candidates: [],
      warnings: [],
    },
    cleanupPlan: null,
    cleanupStatus: null,
    startupSnapshot: {
      scanId: '',
      state: 'Idle',
      inventoryEpoch: 0,
      startedUnixMs: 0,
      completedUnixMs: 0,
      errorMessage: '',
      summary: {
        total: 0,
        registry: 0,
        startupFolders: 0,
        scheduledTasks: 0,
        services: 0,
        protected: 0,
        manageable: 0,
        highImpact: 0,
      },
      items: [],
      warnings: [],
    },
    startupPlan: null,
    startupStatus: null,
    startupHistory: [],
    diagnostics: {
      scanId: '',
      state: 'Idle',
      startedUnixMs: 0,
      completedUnixMs: 0,
      eventWindowDays: 0,
      storage: [],
      memory: null,
      events: [],
      crashes: [],
      cards: [],
      warnings: [],
      providerFaults: [],
    },
    diagnosticHistory: [],
    deepScan: {
      scanId: '', state: 1, status: 1, startedUnixMs: 0, completedUnixMs: 0,
      progress: { totalWeight: 100, completedWeight: 0, percent: 0, completedTasks: 0, totalTasks: 7, activeTasks: 0, skippedTasks: 0, failedTasks: 0, unavailableTasks: 0, currentStageKey: '' },
      factsCount: 0, findings: [], remediationCandidates: [], collectors: [], warnings: [],
      summary: { critical: 0, high: 0, moderate: 0, low: 0, informational: 0, recommendedActions: 0, optionalOptimizations: 0, healthyChecks: 0 },
      metrics: { durationMs: 0, collectorDurationMs: 0, peakActiveTasks: 0, streamedEventCount: 0, persistenceWriteCount: 0, normalizedPayloadBytesEstimate: 0 },
      machineStateFingerprint: '', ruleEngineVersion: '', appVersion: '',
    },
    deepScanHistory: [],
    schedulerEvent: null,
    updateSnapshot: { state: 1, channel: 1, currentVersion: '—', latestRelease: null, stagedRelease: null, progressKnown: false, overallPercent: 0, bytesCompleted: 0, bytesTotal: 0, statusMessageKey: 'update.status.disabled', checkedUnixMs: 0, updatedUnixMs: 0 },
    supportBundleEvent: null,
    performance: {
      capturedUnixMs: 0, intervalMs: 0, cpu: null, power: null, memory: null,
      storage: [], gpu: null, processTop: [], collectorFaults: [],
    },
    performanceWindow: [],
    bottleneckReport: null,
    optimizationStatus: null,
    perfSampling: false,
    timelinePage: null,
    careStatus: null,
    insights: null,
    session: {
      connected: false,
      sessionId: '',
      serviceVersion: '',
      currentSequence: 0,
      replayFloorSequence: 0,
      replayComplete: false,
    },
    serviceLog: [],
    lastKernelSequence: 0,
  };
}

export const streamState = writable<StreamState>(createInitialStreamState());

function routePlan(state: StreamState, plan: Plan): StreamState {
  const next = { ...state, snapshot: { ...state.snapshot, connected: true, activePlan: plan } };
  if (plan.kind === 'DriverInstall') next.installPlan = plan;
  if (plan.kind === 'SystemRepair') next.repairPlan = plan;
  if (plan.kind === 'Cleanup') next.cleanupPlan = plan;
  if (plan.kind === 'Startup') next.startupPlan = plan;
  return next;
}

function applyProgress(state: StreamState, value: ProgressTelemetry): StreamState {
  const next = { ...state };
  if (state.installStatus?.planId === value.planId) {
    next.installStatus = {
      ...state.installStatus,
      stage: value.stage,
      progressKnown: value.progressKnown,
      overallPercent: value.overallPercent,
      currentCandidateId: value.currentItemId,
      detail: value.detail,
      bytesDownloaded: value.bytesCompleted,
      bytesTotal: value.bytesTotal,
    };
  }
  if (state.repairStatus?.planId === value.planId) {
    next.repairStatus = {
      ...state.repairStatus,
      stage: value.stage,
      progressKnown: value.progressKnown,
      overallPercent: value.overallPercent,
      currentStepId: value.currentItemId,
      detail: value.detail,
    };
  }
  if (state.cleanupStatus?.planId === value.planId) {
    next.cleanupStatus = {
      ...state.cleanupStatus,
      stage: value.stage,
      progressKnown: value.progressKnown,
      overallPercent: value.overallPercent,
      currentCandidateId: value.currentItemId,
      detail: value.detail,
    };
  }
  if (state.startupStatus?.planId === value.planId) {
    next.startupStatus = {
      ...state.startupStatus,
      stage: value.stage,
      progressKnown: value.progressKnown,
      overallPercent: value.overallPercent,
      currentItemId: value.currentItemId,
      detail: value.detail,
    };
  }
  return next;
}

export function reduceKernelEvent(state: StreamState, event: UiKernelEvent): StreamState {
  if (event.sequence <= state.lastKernelSequence) return state;
  let next: StreamState = { ...state, lastKernelSequence: event.sequence };
  switch (event.kind) {
    case 'serviceSnapshot': {
      const value = event.payload;
      next.snapshot = { ...value, connected: true };
      break;
    }
    case 'plan':
      next = routePlan(next, event.payload);
      break;
    case 'driverHubSnapshot':
      next.hub = event.payload;
      break;
    case 'driverInstallStatus':
      next.installStatus = event.payload;
      break;
    case 'recoveryHistory':
      next.recoveryEntries = event.payload.entries ?? [];
      break;
    case 'repairAssessment':
      next.repairAssessment = event.payload;
      break;
    case 'systemRepairStatus':
      next.repairStatus = event.payload;
      break;
    case 'cleanupSnapshot':
      next.cleanupSnapshot = event.payload;
      break;
    case 'cleanupStatus':
      next.cleanupStatus = event.payload;
      break;
    case 'startupSnapshot':
      next.startupSnapshot = event.payload;
      break;
    case 'startupStatus':
      next.startupStatus = event.payload;
      break;
    case 'startupHistory':
      next.startupHistory = event.payload.entries ?? [];
      break;
    case 'diagnosticsSnapshot':
      next.diagnostics = event.payload;
      break;
    case 'diagnosticsHistory':
      next.diagnosticHistory = event.payload.entries ?? [];
      break;
    case 'deepScanSnapshot':
      next.deepScan = event.payload;
      break;
    case 'performanceSnapshot':
      next.performance = event.payload;
      {
        const window = [...state.performanceWindow, event.payload];
        next.performanceWindow = window.length > 300 ? window.slice(window.length - 300) : window;
      }
      // `perfSampling` is whether the SERVICE's background sampler is running,
      // and only `start_perf_sampling` / `stop_perf_sampling` decide that. A
      // snapshot event arrives from every `get_performance_snapshot` — including
      // the Overview's own read loop, which starts no sampler — so inferring it
      // here made the Performance screen offer "Stop" for a thread that does not
      // exist. See §51.1.
      break;
    case 'bottleneckReport':
      next.bottleneckReport = event.payload;
      break;
    case 'optimizationStatus':
      next.optimizationStatus = event.payload;
      break;
    case 'timelinePage':
      next.timelinePage = event.payload;
      break;
    case 'careStatus':
      next.careStatus = event.payload;
      break;
    case 'insights':
      next.insights = event.payload;
      break;
    case 'progressTelemetry':
      next = applyProgress(next, event.payload);
      break;
    case 'scheduler':
      next.schedulerEvent = event.payload;
      break;
    case 'updateSnapshot':
      next.updateSnapshot = event.payload;
      break;
    case 'supportBundle':
      next.supportBundleEvent = event.payload;
      break;
    case 'consentIntent':
    case 'mutationLease':
    case 'unknown':
      // These events affect security/diagnostic observability but do not own a visual domain slice.
      break;
    default:
      return assertNeverEvent(event);
  }
  // Recorded after the switch so an event that fails to route is never logged as
  // if it had been handled.
  next.serviceLog = [...state.serviceLog, event].slice(-SERVICE_LOG_LIMIT);
  return next;
}

function assertNeverEvent(event: never): StreamState {
  throw new Error(`Unhandled kernel event: ${JSON.stringify(event)}`);
}

export function applyKernelEvent(event: UiKernelEvent): void {
  streamState.update((state) => reduceKernelEvent(state, event));
}

export function applySessionState(session: UiSessionState): void {
  streamState.update((state) => ({
    ...state,
    session,
    snapshot: {
      ...state.snapshot,
      connected: session.connected,
      serviceVersion: session.connected && session.serviceVersion ? session.serviceVersion : state.snapshot.serviceVersion,
      health: session.connected ? 'PlatformReady' : 'Offline',
    },
  }));
}

export function applyStreamReset(reset: UiStreamReset): void {
  const previous = get(streamState);
  const fresh = createInitialStreamState();
  streamState.set({
    ...fresh,
    snapshot: {
      ...fresh.snapshot,
      connected: previous.snapshot.connected,
      serviceVersion: previous.snapshot.serviceVersion,
      health: previous.snapshot.health,
    },
    session: previous.session,
    lastKernelSequence: reset.currentSequence,
  });
}

export function patchStreamState(patch: Partial<StreamState>): void {
  streamState.update((state) => ({ ...state, ...patch }));
}

export function patchSnapshot(snapshot: Snapshot): void {
  streamState.update((state) => {
    let next = { ...state, snapshot };
    const active = snapshot.activePlan;
    if (active) next = routePlan(next, active);
    return next;
  });
}

export function applyPerformanceWindow(samples: PerfSnapshot[]): void {
  streamState.update((state) => ({
    ...state,
    performanceWindow: samples.slice(-300),
    performance: samples.length > 0 ? samples[samples.length - 1] : state.performance,
  }));
}
