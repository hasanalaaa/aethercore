export type Plan = {
    id:string; kind:string; title:string; state:string; digest:string; risk:string; createdUnixMs:number;
    updatedUnixMs:number; requiresAuthorization:boolean; consentReadyUntilUnixMs:number;
    actionCount:number; inventoryEpoch:number; scanId:string
  };
export type Snapshot = {
    connected:boolean; serviceVersion:string; health:string; serverTimeUnixMs:number;
    journalEventCount:number; activePlan:Plan|null
  };
export type InstalledDriver = { provider:string; version:string; infPath:string; date:string };
export type GpuManagement = {
    vendor:string; providerId:string; appInstalled:boolean; appName:string; officialUrl:string;
    managementStatus:string; updateAvailability:string
  };
export type DriverManagementAuthority = {
    providerId:string; authorityType:string; displayName:string; officialAuthority:string; officialUrl:string;
    availability:string; updateAvailability:string; installationMode:string; updateEvidenceCandidateId:string
  };
export type DriverCandidate = {
    candidateId:string; updateId:string; revision:number; title:string; provider:string;
    manufacturer:string; model:string; driverClass:string; matchedHardwareId:string;
    matchQuality:string; driverDateIso:string; minDownloadBytes:number; maxDownloadBytes:number;
    targetVersion:string; targetVersionSource:string; selectable:boolean;
    selectedByDefault:boolean; vendorManaged:boolean; firmwareManaged:boolean; selectionPolicy:string;
    authorityType:string; authorityProviderId:string; authorityName:string; officialSource:string;
    applicability:string; trustState:string; recommendationState:string; recommendationReasons:string[];
    acquisitionMode:string; installationMode:string; officialSupportUrl:string; recommended:boolean
  };
export type DriverDevice = {
    instanceId:string; displayName:string; description:string; className:string; classGuid:string;
    manufacturer:string; enumerator:string; location:string; hardwareIds:string[]; compatibleIds:string[];
    rawStatus:number; problemCode:number; hasProblem:boolean; missingDriver:boolean;
    driver:InstalledDriver|null; deviceState:string; gpu:GpuManagement|null; displayManaged:boolean;
    recommendedCandidateId:string; updateStatus:string; authorityCoverage:string; candidates:DriverCandidate[];
    requiredAuthorities:string[]; evaluatedAuthorities:string[]; unavailableAuthorities:string[];
    unsupportedAuthorities:string[]; manualAuthorities:string[]; managementAuthorities:DriverManagementAuthority[]
  };
export type HubSummary = {
    deviceCount:number; missingDriverCount:number; problemDeviceCount:number; updateOfferCount:number;
    matchedDeviceCount:number; matchedCandidateCount:number; selectableUpdateCount:number;
    vendorManagedGpuCount:number; vendorManagedDisplayCount:number; recommendedUpdateCount:number;
    optionalUpdateCount:number; vendorManagedUpdateCount:number; statusUnknownCount:number; managementAuthorityCount:number
  };
export type DriverHub = {
    scanId:string; state:string; inventoryEpoch:number; startedUnixMs:number; completedUnixMs:number;
    errorMessage:string; summary:HubSummary; devices:DriverDevice[];
    unmatchedOffers:unknown[]; warnings:string[]; authorityCoverage:string; providerStatus:string[]
  };
export type DriverInstallItemStatus = {
    candidateId:string; instanceId:string; title:string; stage:string; progressKnown:boolean;
    progressPercent:number; resultCode:string; hresult:number; rebootRequired:boolean; verified:boolean;
    beforeVersion:string; afterVersion:string; beforeProblemCode:number; afterProblemCode:number;
    backupPath:string; detail:string
  };
export type DriverInstallStatus = {
    planId:string; planState:string; stage:string; progressKnown:boolean; overallPercent:number;
    currentCandidateId:string; bytesDownloaded:number; bytesTotal:number; detail:string; rebootRequired:boolean; restorePointVerified:boolean;
    restorePointSequence:number; backupRoot:string; mutationStarted:boolean; recoveryRequired:boolean;
    failureMessage:string; startedUnixMs:number; updatedUnixMs:number; completedUnixMs:number;
    items:DriverInstallItemStatus[]
  };
export type RecoveryEntry = {
    seq:number; planId:string; severity:string; kind:string; summary:string; detail:string;
    restorePointSequence:number; backupRoot:string; createdUnixMs:number
  };
export type RepairCheck = { id:string; title:string; stage:string; resultCode:string; exitCode:number; detail:string; logHint:string };
export type RepairFact = { id:string; domain:string; state:string; resource:string; evidenceCode:string; technicalCode:string; detail:string; observedUnixMs:number; confidence:string };
export type RepairDiagnosis = { id:string; code:string; role:string; domain:string; confidence:string; scope:string; evidenceIds:string[]; uncertainty:string; ruleVersion:string };
export type RepairNode = { id:string; action:string; safety:string; dependencies:string[]; targetResource:string; diagnosisIds:string[]; verification:string; reversibility:string; requiresExplicitConsent:boolean; requiresRecoveryProtection:boolean; rebootBoundaryAfter:boolean; executableAutomatically:boolean };
export type RepairGraph = { schema:string; valid:boolean; invalidReason:string; nodes:RepairNode[]; deterministicOrder:string[]; digestSha256:string };
export type RecoveryReadiness = { systemRestore:string; restorePointCreation:string; winRe:string; journalRecovery:string; driverRollback:string };
export type RepairIntelligence = { schema:string; observationId:string; machineStateFingerprint:string; facts:RepairFact[]; diagnoses:RepairDiagnosis[]; recovery:RecoveryReadiness; graph:RepairGraph };
export type RepairAssessment = {
    assessmentId:string; state:string; startedUnixMs:number; completedUnixMs:number; errorMessage:string;
    systemVolume:string; checks:RepairCheck[]; intelligence:RepairIntelligence|null
  };
export type SystemRepairStatus = {
    planId:string; planState:string; stage:string; progressKnown:boolean; overallPercent:number;
    currentStepId:string; detail:string; mutationStarted:boolean; recoveryRequired:boolean; failureMessage:string;
    outcome:string; repairGraphDigest:string; machineStateFingerprint:string; safetyTier:string; rebootRequired:boolean; verificationState:string;
    startedUnixMs:number; updatedUnixMs:number; completedUnixMs:number; steps:RepairCheck[]
  };
export type CleanupCandidate = {
    candidateId:string; provider:string; title:string; description:string; reclaimableBytes:number; fileCount:number;
    selectedByDefault:boolean; requiresExplicitConfirmation:boolean; truncated:boolean; specialKind:string
  };
export type CleanupSnapshot = {
    scanId:string; state:string; inventoryEpoch:number; startedUnixMs:number; completedUnixMs:number; errorMessage:string;
    totalReclaimableBytes:number; totalFileCount:number; candidates:CleanupCandidate[]; warnings:string[]
  };
export type CleanupItemStatus = { itemId:string; title:string; stage:string; resultCode:string; bytesAffected:number; detail:string };
export type CleanupStatus = {
    planId:string; planState:string; stage:string; progressKnown:boolean; overallPercent:number; currentCandidateId:string;
    detail:string; mutationStarted:boolean; recoveryRequired:boolean; failureMessage:string; reclaimedBytes:number; skippedBytes:number;
    startedUnixMs:number; updatedUnixMs:number; completedUnixMs:number; items:CleanupItemStatus[]
  };
export type StartupSummary = { total:number; registry:number; startupFolders:number; scheduledTasks:number; services:number; protected:number; manageable:number; highImpact:number };
export type StartupItem = {
    itemId:string; kind:string; scope:string; displayName:string; publisher:string; command:string; source:string; enabled:boolean;
    manageable:boolean; protected:boolean; protectionReason:string; impact:string; confidence:string; evidenceDetail:string; recommendation:string; serviceChange:boolean
  };
export type StartupSnapshot = { scanId:string; state:string; inventoryEpoch:number; startedUnixMs:number; completedUnixMs:number; errorMessage:string; summary:StartupSummary; items:StartupItem[]; warnings:string[] };
export type StartupDecision = 'Unreviewed' | 'KeepEnabled' | 'Disable';
export type StartupExecutionItem = { itemId:string; displayName:string; kind:string; stage:string; resultCode:string; detail:string };
export type StartupStatus = { planId:string; planState:string; stage:string; progressKnown:boolean; overallPercent:number; currentItemId:string; detail:string; mutationStarted:boolean; recoveryRequired:boolean; failureMessage:string; startedUnixMs:number; updatedUnixMs:number; completedUnixMs:number; items:StartupExecutionItem[] };
export type StartupHistoryEntry = { changeId:string; originChangeId:string; planId:string; itemId:string; kind:string; displayName:string; direction:string; state:string; detail:string; createdUnixMs:number; updatedUnixMs:number; restoredUnixMs:number; restorable:boolean };
export type AtaSmartAttribute = { id:number; current:number; worst:number; rawValueDecimal:string; rawValueHex:string };
export type StorageReliability = { hasTemperature:boolean; temperatureC:number; hasTemperatureMax:boolean; temperatureMaxC:number; hasWear:boolean; wearPercentUsed:number; hasPowerOnHours:boolean; powerOnHours:number; hasReadErrorsUncorrected:boolean; readErrorsUncorrected:number; hasWriteErrorsUncorrected:boolean; writeErrorsUncorrected:number; hasNvmeCriticalWarning:boolean; nvmeCriticalWarning:number; hasNvmeAvailableSpare:boolean; nvmeAvailableSparePercent:number; hasNvmePercentageUsed:boolean; nvmePercentageUsed:number; nvmeMediaErrors:string; nvmeUnsafeShutdowns:string; nvmeErrorLogEntries:string; hasReadLatencyMax:boolean; readLatencyMaxMs:number; hasWriteLatencyMax:boolean; writeLatencyMaxMs:number; hasFlushLatencyMax:boolean; flushLatencyMaxMs:number };
export type StorageTelemetry = { deviceId:string; friendlyName:string; firmwareVersion:string; serialNumber:string; busType:string; mediaType:string; sizeBytes:number; windowsHealthStatus:string; operationalStatus:string[]; reliability:StorageReliability|null; severity:string; summary:string; reasons:string[]; sourceNotes:string[]; ataSmartAttributes:AtaSmartAttribute[] };
export type MemoryTelemetry = { totalPhysicalBytes:number; availablePhysicalBytes:number; memoryLoadPercent:number; pressureLabel:string; pressureExplanation:string };
export type HardwareEvent = { eventId:number; provider:string; recordedUnixMs:number; category:string; severity:string; confidence:string; summary:string; detail:string };
export type CrashRecord = { crashId:string; recordedUnixMs:number; hasBugcheckCode:boolean; bugcheckCode:number; bugcheckHex:string; parameters:string[]; dumpFile:string; dumpSizeBytes:number; source:string; confidence:string; summary:string };
export type DiagnosticCard = { cardId:string; domain:string; severity:string; confidence:string; title:string; summary:string; evidence:string[]; actions:string[] };
export type ProviderFault = { provider:string; operation:string; detail:string; kind:string; kindCode:number };
export type DiagnosticsSnapshot = { scanId:string; state:string; startedUnixMs:number; completedUnixMs:number; eventWindowDays:number; storage:StorageTelemetry[]; memory:MemoryTelemetry|null; events:HardwareEvent[]; crashes:CrashRecord[]; cards:DiagnosticCard[]; warnings:string[]; providerFaults:ProviderFault[] };
export type DiagnosticHistoryEntry = { scanId:string; state:string; collectedUnixMs:number; warningCount:number; cardCount:number };
export type ConsentIntentEvent = { intentId:string; planId:string; planDigest:string; title:string; risk:string; actionCount:number; expiresUnixMs:number; riskCode:number };
export type MutationLeaseEvent = { leaseId:string; workload:number; planId:string; state:number; acquiredUnixMs:number; changedUnixMs:number };
export type PcMessageArg = { key:string; value:string };
export type PcResourceRef = { kind:string; stableId:string; displayName:string };
export type PcEvidenceRef = { factId:string; kind:string; source:string; observedUnixMs:number; technicalValue:string };
export type PcResolutionEvidence = { scope:string; collectorState:number; observedUnixMs:number; evidenceFactIds:string[] };
export type PcCorrelationExplanation = { strength:string; timeDistanceMs:number; sharedScope:string; rationaleKey:string; contributingFactIds:string[]; conflictingEvidenceKeys:string[] };
export type PcFinding = {
  id:string; code:string; domain:number; severity:number; confidence:number; titleKey:string; summaryKey:string; technicalKey:string;
  messageArgs:PcMessageArg[]; evidence:PcEvidenceRef[]; affectedResource:PcResourceRef|null; firstObservedUnixMs:number; lastObservedUnixMs:number;
  lifecycle:string; remediationAvailable:boolean; remediationSafety:number; rebootRequirement:string; privilegeRequirement:string; automaticEligible:boolean;
  reversibility:string; estimatedImpact:string; uncertaintyKey:string; ignored:boolean; ruleId:string; ruleVersion:number;
  verificationStatus:string; resolutionAuthority:string[]; hasResolvedAt:boolean; resolvedAtUnixMs:number; resolutionScanId:string; resolutionReasonKey:string;
  resolutionEvidence:PcResolutionEvidence[]; correlation:PcCorrelationExplanation|null;
};
export type PcRemediationCandidate = { actionId:string; findingId:string; actionType:string; descriptionKey:string; authority:string; privilege:string; safety:number; reversibility:string; rebootRequirement:string; expectedEffectKey:string; preconditions:string[]; verificationMethodKey:string; conflicts:string[]; durationCategory:string; automaticEligible:boolean };
export type PcCollectorStatus = { id:string; state:number; stageKeys:string[]; startedUnixMs:number; completedUnixMs:number; detail:string };
export type PcScanProgress = { totalWeight:number; completedWeight:number; percent:number; completedTasks:number; totalTasks:number; activeTasks:number; skippedTasks:number; failedTasks:number; unavailableTasks:number; currentStageKey:string };
export type PcFindingSummary = { critical:number; high:number; moderate:number; low:number; informational:number; recommendedActions:number; optionalOptimizations:number; healthyChecks:number };
export type PcScanMetrics = { durationMs:number; collectorDurationMs:number; peakActiveTasks:number; streamedEventCount:number; persistenceWriteCount:number; normalizedPayloadBytesEstimate:number };
export type DeepScanSnapshot = { scanId:string; state:number; status:number; startedUnixMs:number; completedUnixMs:number; progress:PcScanProgress|null; factsCount:number; findings:PcFinding[]; remediationCandidates:PcRemediationCandidate[]; collectors:PcCollectorStatus[]; warnings:string[]; summary:PcFindingSummary|null; metrics:PcScanMetrics|null; machineStateFingerprint:string; ruleEngineVersion:string; appVersion:string };
export type DeepScanHistoryEntry = { scanId:string; state:number; status:number; completedUnixMs:number; durationMs:number; findingCount:number; unavailableCollectorCount:number; machineStateFingerprint:string };
export type PcRemediationPlan = { planId:string; scanId:string; digest:string; createdUnixMs:number; immutable:boolean; actions:PcRemediationCandidate[] };
// ---------------------------------------------------------------------------
// Phase 20 — performance intelligence
// ---------------------------------------------------------------------------
export type PerfCollectorFault = { collector:string; kind:string; detail:string };
export type CpuSample = { perProcessorBusyBp:number[]; totalBusyBp:number; dpcIsrBusyBp:number; contextSwitchesPerSec:number; processorQueueLengthX100:number };
export type PowerSample = { throttleActive:boolean; throttleReason:number; limitReasonsRaw:number; hasTemperature:boolean; temperatureC:number };
export type MemorySample = { totalPhysicalBytes:number; availablePhysicalBytes:number; standbyCacheBytes:number; modifiedPageListBytes:number; commitBytes:number; commitLimitBytes:number; hardFaultsPerSec:number; softFaultsPerSec:number; memoryLoadPercent:number };
export type StorageQueueSample = { deviceId:string; friendlyName:string; activeTimeBp:number; queueDepthX100:number; avgTransferLatencyUs:number; readBytesPerSec:number; writeBytesPerSec:number; totalSpaceBytes:number; freeSpaceBytes:number };
export type GpuEngineSample = { engineName:string; utilizationBp:number };
export type GpuSample = { adapterId:string; adapterName:string; dedicatedUsedBytes:number; dedicatedTotalBytes:number; sharedUsedBytes:number; engines:GpuEngineSample[]; frametimeJitterUs:number; compositorLagDetected:boolean };
export type ProcessCpuTopEntry = { pid:number; name:string; cpuBusyBp:number; readBytesPerSec:number; writeBytesPerSec:number; workingSetBytes:number };
export type PerfSnapshot = {
  capturedUnixMs:number; intervalMs:number;
  cpu:CpuSample | null; power:PowerSample | null; memory:MemorySample | null;
  storage:StorageQueueSample[]; gpu:GpuSample | null;
  processTop:ProcessCpuTopEntry[]; collectorFaults:PerfCollectorFault[]
};
export type PerformanceWindowResponse = { samples: PerfSnapshot[] };
export type PerfMessageArg = { key:string; value:string };
export type BottleneckEvidenceRef = { factKey:string; observedValue:number; threshold:number; observedUnixMs:number };
export type BottleneckFinding = {
  id:string; code:string; role:number; confidence:number;
  causedByFindingIds:string[]; titleKey:string; summaryKey:string;
  messageArgs:PerfMessageArg[]; evidence:BottleneckEvidenceRef[];
  applicableActionKinds:string[]; firstObservedUnixMs:number; lastObservedUnixMs:number
};
export type BottleneckReport = {
  reportId:string; generatedUnixMs:number; analyzedSampleCount:number; analysisWindowMs:number;
  findings:BottleneckFinding[]; digestSha256:string; ruleEngineVersion:string
};
export type OptimizationCandidate = {
  candidateId:string; kind:number; findingIds:string[]; titleKey:string; descriptionKey:string;
  reversibility:number; expectedEffectMetricKeys:string[]; targetPids:number[];
  requiresExplicitConsent:boolean
};
export type OptimizationPlanSnapshot = {
  planId:string; reportId:string; digestSha256:string; createdUnixMs:number;
  immutable:boolean; candidates:OptimizationCandidate[]
};
export type OptimizationExecutionItem = { candidateId:string; stage:string; resultCode:string; detail:string; verified:boolean };
export type OptimizationStatus = {
  planId:string; planState:string; stage:string; progressKnown:boolean; overallPercent:number;
  currentCandidateId:string; detail:string; mutationStarted:boolean; recoveryRequired:boolean;
  failureMessage:string; startedUnixMs:number; updatedUnixMs:number; completedUnixMs:number;
  items:OptimizationExecutionItem[]
};

// ---------------------------------------------------------------------------
// Phase 21 — Timeline Intelligence & recurrence reasoning
// ---------------------------------------------------------------------------
export type TimelineEventClass =
  | 'TIMELINE_EVENT_CLASS_UNSPECIFIED'
  | 'TIMELINE_EVENT_CLASS_OPERATION'
  | 'TIMELINE_EVENT_CLASS_FINDING'
  | 'TIMELINE_EVENT_CLASS_VERIFICATION'
  | 'TIMELINE_EVENT_CLASS_RECOVERY'
  | 'TIMELINE_EVENT_CLASS_ESCALATION';
export type TimelineOutcomeKind =
  | 'TIMELINE_OUTCOME_UNSPECIFIED'
  | 'TIMELINE_OUTCOME_SUCCEEDED'
  | 'TIMELINE_OUTCOME_FAILED'
  | 'TIMELINE_OUTCOME_NEUTRAL';
export type RecurrenceConfidenceKind =
  | 'RECURRENCE_CONFIDENCE_UNSPECIFIED'
  | 'RECURRENCE_CONFIDENCE_WEAK'
  | 'RECURRENCE_CONFIDENCE_MODERATE'
  | 'RECURRENCE_CONFIDENCE_STRONG';
export type TimelineEntry = {
  sourceId:string; class:TimelineEventClass; domain:string; code:string;
  outcome:TimelineOutcomeKind; observedUnixMs:number; semanticIdentitySha256:string
};
export type TimelineResponse = {
  entries:TimelineEntry[]; hasMore:boolean; nextBeforeSequence:number;
  digestSha256:string; duplicatesCollapsed:number
};
export type RecurrenceEvidence = { sourceId:string; observedUnixMs:number; gapFromPreviousMs:number };
export type RecurrencePattern = {
  semanticIdentitySha256:string; class:TimelineEventClass; domain:string; code:string;
  confidence:RecurrenceConfidenceKind; occurrenceCount:number;
  firstObservedUnixMs:number; lastObservedUnixMs:number; meanGapMs:number;
  evidence:RecurrenceEvidence[]
};
export type RecurrencePatternsResponse = { patterns:RecurrencePattern[]; digestSha256:string };

// ---------------------------------------------------------------------------
// Phase 22 — One-Click Care orchestration
// ---------------------------------------------------------------------------
export type CareStepReport = {
  stepIndex:number; domainPlanId:string; domainKind:string; safetyLevel:number;
  state:string; outcome:string; domainVerificationState:string; failureMessageKey:string
};
export type CareRunStatus = {
  runId:string; state:string; stage:string; sessionConsentGranted:boolean;
  planDigestSha256:string; steps:CareStepReport[]; updatedUnixMs:number; summaryKey:string
};

// Phase 23 — Local Intelligence (advisory-only)
export type InsightCitation = { evidenceId:string; surface:'bottleneckReport'|'repairDiagnosis'|'timelinePattern'|'maintenanceHistory'|'securityFinding' };
export type Insight = {
  /** Session handle assigned by the service; the only value `dismiss_insight` matches on. */
  id:string;
  schemaVersion:number; summaryKey:string; explanation:string;
  confidence:'Weak'|'Moderate'|'Strong'; citations:InsightCitation[];
  engine:'localModel'|'ruleFallback'
};
export type InsightsResponse = {
  engineLabel:'localModel'|'ruleFallback'|'disabled'; insights:Insight[]
};

type KernelEvent<K extends string, P> = { sequence:number; emittedUnixMs:number; kind:K; planId:string; payload:P };
export type UiKernelEvent =
  | KernelEvent<'serviceSnapshot', Omit<Snapshot, 'connected'>>
  | KernelEvent<'plan', Plan>
  | KernelEvent<'driverHubSnapshot', DriverHub>
  | KernelEvent<'driverInstallStatus', DriverInstallStatus>
  | KernelEvent<'recoveryHistory', { entries: RecoveryEntry[] }>
  | KernelEvent<'repairAssessment', RepairAssessment>
  | KernelEvent<'systemRepairStatus', SystemRepairStatus>
  | KernelEvent<'cleanupSnapshot', CleanupSnapshot>
  | KernelEvent<'cleanupStatus', CleanupStatus>
  | KernelEvent<'startupSnapshot', StartupSnapshot>
  | KernelEvent<'startupStatus', StartupStatus>
  | KernelEvent<'startupHistory', { entries: StartupHistoryEntry[] }>
  | KernelEvent<'diagnosticsSnapshot', DiagnosticsSnapshot>
  | KernelEvent<'diagnosticsHistory', { entries: DiagnosticHistoryEntry[] }>
  | KernelEvent<'consentIntent', ConsentIntentEvent>
  | KernelEvent<'mutationLease', MutationLeaseEvent>
  | KernelEvent<'progressTelemetry', ProgressTelemetry>
  | KernelEvent<'scheduler', SchedulerEvent>
  | KernelEvent<'updateSnapshot', UpdateSnapshot>
  | KernelEvent<'supportBundle', SupportBundleEvent>
  | KernelEvent<'deepScanSnapshot', DeepScanSnapshot>
  | KernelEvent<'performanceSnapshot', PerfSnapshot>
  | KernelEvent<'bottleneckReport', BottleneckReport>
  | KernelEvent<'optimizationStatus', OptimizationStatus>
  | KernelEvent<'timelinePage', TimelineResponse>
  | KernelEvent<'careStatus', CareRunStatus>
  | KernelEvent<'insights', InsightsResponse>
  | KernelEvent<'unknown', null>;
export type UiSessionState = { connected:boolean; sessionId:string; serviceVersion:string; currentSequence:number; replayFloorSequence:number; replayComplete:boolean };
export type UiStreamReset = { reason:string; currentSequence:number; replayFloorSequence:number; messageKey:string };
export type ProgressTelemetry = { planId:string; stage:string; progressKnown:boolean; overallPercent:number; currentItemId:string; detail:string; bytesCompleted:number; bytesTotal:number };
export type DriverFilter = 'All' | 'Updates' | 'Problems' | 'Missing' | 'Display';


export type SchedulerEvent = { workload:string; state:number; reason:string; changedUnixMs:number; evidenceCount:number; warningCount:number };

export type UpdateRelease = { releaseId:string; version:string; channel:number; publishedUnixMs:number; notesMessageKey:string; minimumWindowsBuild:number; sizeBytes:number; sha256:string; packageKind:number };
export type UpdateSnapshot = { state:number; channel:number; currentVersion:string; latestRelease:UpdateRelease|null; stagedRelease:UpdateRelease|null; progressKnown:boolean; overallPercent:number; bytesCompleted:number; bytesTotal:number; statusMessageKey:string; checkedUnixMs:number; updatedUnixMs:number };
export type SupportPrivacyReport = { userPathRedactions:number; accountIdentifierRedactions:number; hardwareSerialRedactions:number; emailRedactions:number };
export type SupportPreviewSection = { fileName:string; displayKey:string; sizeBytes:number };
export type SupportBundlePreview = { previewId:string; expiresUnixMs:number; sections:SupportPreviewSection[]; privacy:SupportPrivacyReport|null; estimatedSizeBytes:number };
export type SupportBundleEvent = { state:number; previewId:string; bundleId:string; sizeBytes:number; sha256:string; changedUnixMs:number };
export type SupportExportResult = { path:string; sha256:string; verificationFingerprintSha256:string };
