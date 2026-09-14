import type { DriverCandidate, DriverDevice } from '../contracts';
import { hasMessageKey, t, td, type Locale, type MessageKey } from './runtime';

const stateKeys: Record<string, MessageKey> = {
  Queued:'state.Queued', Interrupted:'state.Interrupted', Attention:'state.Attention', BackedUp:'state.BackedUp', BackingUpDrivers:'state.BackingUpDrivers', BackupNotApplicable:'state.BackupNotApplicable', Downloading:'state.Downloading', FailedAfterMutation:'state.FailedAfterMutation', FailedBeforeMutation:'state.FailedBeforeMutation', FailedSafe:'state.FailedSafe', FailedVerification:'state.FailedVerification', Installed:'state.Installed', Installing:'state.Installing', Verified:'state.Verified',
  Idle: 'state.Idle', Scanning: 'state.Scanning', InventoryScanning: 'state.InventoryScanning',
  UpdateSearching: 'state.UpdateSearching', Matching: 'state.Matching', Ready: 'state.Ready', Failed: 'state.Failed',
  Collecting: 'state.Collecting', AwaitingAuthorization: 'state.AwaitingAuthorization', Preflight: 'state.Preflight',
  Protected: 'state.Protected', Executing: 'state.Executing', Verifying: 'state.Verifying', RebootPending: 'state.RebootPending',
  Completed: 'state.Completed', RecoveryRequired: 'state.RecoveryRequired', Prepared: 'state.Prepared', Applied: 'state.Applied',
  AppliedRecovered: 'state.AppliedRecovered', NoChange: 'state.NoChange', Restored: 'state.Completed',
};
const riskKeys: Record<string, MessageKey> = { Low:'risk.Low', Medium:'risk.Medium', High:'risk.High', Amber:'risk.Amber' };
const severityKeys: Record<string, MessageKey> = { Normal:'severity.Normal', Info:'severity.Info', Attention:'severity.Attention', ActionRequired:'severity.ActionRequired', Warning:'severity.Warning', warning:'severity.Warning', Unknown:'severity.Unknown' };
const confidenceKeys: Record<string, MessageKey> = {
  HighEvidence:'confidence.HighEvidence', 'HighEvidence/RootCauseUnknown':'confidence.HighEvidence/RootCauseUnknown',
  'EventHigh/CauseLow':'confidence.EventHigh/CauseLow', EventOnly:'confidence.EventOnly', High:'confidence.High', Medium:'confidence.Medium', Low:'confidence.Low',
  ReportedMetricEvidence:'confidence.ReportedMetricEvidence', EvidenceUnavailable:'confidence.EvidenceUnavailable', LoggedHardwareEvidence:'confidence.LoggedHardwareEvidence',
  LogWindowOnly:'confidence.LogWindowOnly', CurrentOSMetric:'confidence.CurrentOSMetric', HeaderEvidence:'confidence.HeaderEvidence', MetadataOnly:'confidence.MetadataOnly',
  LoggedCrashEvidence:'confidence.LoggedCrashEvidence', InsufficientEvidence:'confidence.InsufficientEvidence',
};
const impactKeys: Record<string, MessageKey> = { High:'impact.High', Medium:'impact.Medium', Low:'impact.Low', Unknown:'impact.Unknown' };
const kindKeys: Record<string, MessageKey> = { RegistryRun:'kind.RegistryRun', RegistryRunOnce:'kind.RegistryRunOnce', StartupFolder:'kind.StartupFolder', ScheduledTask:'kind.ScheduledTask', Service:'kind.Service', SystemRepairInterrupted:'recoveryKind.SystemRepairInterrupted', CleanupInterrupted:'recoveryKind.CleanupInterrupted', StartupRecovery:'recoveryKind.StartupRecovery', 'interrupted-mutation':'recoveryKind.interrupted-mutation', 'install-failure':'recoveryKind.install-failure', 'restore-transaction-close-failure':'recoveryKind.restore-transaction-close-failure', 'verification-failure':'recoveryKind.verification-failure', 'safe-failure':'recoveryKind.safe-failure' };
const directionKeys: Record<string, MessageKey> = { Disable:'direction.Disable', Restore:'direction.Restore' };
const domainKeys: Record<string, MessageKey> = { Storage:'domain.Storage', Memory:'domain.Memory', Hardware:'domain.Hardware', Crash:'domain.Crash' };
const recommendationKeys: Record<string, MessageKey> = { Review:'recommendation.Review', 'Keep enabled':'recommendation.Keep enabled' };
const planKindKeys: Record<string, MessageKey> = { DriverInstall:'planKind.DriverInstall', SystemRepair:'planKind.SystemRepair', Cleanup:'planKind.Cleanup', Startup:'planKind.Startup' };

export function localizeState(value: string, locale: Locale): string { return stateKeys[value] ? td(stateKeys[value], locale) : value; }
export function localizeRisk(value: string, locale: Locale): string { return riskKeys[value] ? td(riskKeys[value], locale) : value; }
export function localizeSeverity(value: string, locale: Locale): string { return severityKeys[value] ? td(severityKeys[value], locale) : value; }
export function localizeConfidence(value: string, locale: Locale): string { return confidenceKeys[value] ? td(confidenceKeys[value], locale) : value; }
export function localizeImpact(value: string, locale: Locale): string { return impactKeys[value] ? td(impactKeys[value], locale) : value; }
export function localizeKind(value: string, locale: Locale): string { return kindKeys[value] ? td(kindKeys[value], locale) : value; }
export function localizeDirection(value: string, locale: Locale): string { return directionKeys[value] ? td(directionKeys[value], locale) : value; }
export function localizeDomain(value: string, locale: Locale): string { return domainKeys[value] ? td(domainKeys[value], locale) : value; }
export function localizeRecommendation(value: string, locale: Locale): string { return recommendationKeys[value] ? td(recommendationKeys[value], locale) : value; }
export function localizePlanKind(value: string, locale: Locale): string { return planKindKeys[value] ? td(planKindKeys[value], locale) : value; }
export function localizeMatchQuality(value: string, locale: Locale): string { const key = ({ 'Hardware ID':'drivers.match.Hardware ID', 'Compatible ID':'drivers.match.Compatible ID' } as const)[value as 'Hardware ID'|'Compatible ID']; return key ? td(key, locale) : value; }

const exactOwnedText: Record<string, MessageKey> = {
  'Startup change needs review':'startup.failure',
  'Authorized repair queued':'tech.repair.queued',
  'Checking servicing safety':'tech.repair.preflight',
  'Repair workflow is frozen; waiting for the servicing mutation barrier':'tech.repair.frozen',
  'Running supported Windows repair tools':'tech.repair.executing',
  'Verifying component store and protected files':'tech.repair.verifying',
  'Repair workflow completed and verification commands finished successfully.':'tech.repair.completed',
  'Repair stopped. No command will be replayed automatically.':'tech.repair.stopped',
  'Repair was interrupted after mutation began. AetherCore will not replay DISM/SFC automatically.':'tech.repair.interruptedAfter',
  'Repair was interrupted before mutation began. No repair command was replayed.':'tech.repair.interruptedBefore',
  'Service restart interrupted repair':'tech.repair.restartFailure',
  'Component store quick check':'tech.repair.componentQuick',
  'Protected system files':'tech.repair.protectedFiles',
  'System volume online scan':'tech.repair.volumeScan',
  'DISM CheckHealth':'tech.repair.dismCheck',
  'DISM ScanHealth':'tech.repair.dismScan',
  'DISM RestoreHealth':'tech.repair.dismRestore',
  'System File Checker repair':'tech.repair.sfcRepair',
  'Verify component store':'tech.repair.verifyComponent',
  'Verify protected system files':'tech.repair.verifyFiles',
  'Verify system volume':'tech.repair.verifyVolume',
  'Event Viewer → Application → Chkdsk':'tech.repair.eventViewerHint',
  "Windows Update service":'tech.repair.serviceTitle',
  "The diagnosis-scoped Windows Update service is running.":'tech.repair.serviceRunning',
  "Windows Update discovery failed and its required wuauserv service is not running. AetherCore may offer only this targeted service start; it will not reset unrelated services.":'tech.repair.serviceStopped',
  "Start required Windows Update service":'tech.repair.startServiceTitle',
  "AetherCore requested the fixed diagnosis-scoped wuauserv service to start. Verification will query the service state separately.":'tech.repair.serviceStartRequested',
  "Authorized evidence-backed repair queued":'tech.repair.queuedEvidence',
  "Windows servicing state":'tech.update.servicingTitle',
  "Windows reports that another servicing installation is active. AetherCore will not compete with it.":'tech.update.servicingBusy',
  "Windows Update Agent reports that a restart is required before another installation can safely begin.":'tech.update.rebootPending',
  "Windows Update Agent discovery completed successfully, but the pending-update count could not be read; the reported 0 is not a measurement.":'tech.update.countUnavailable',
  "Windows Update Agent is only available on Windows":'tech.update.windowsOnly',
  "Windows Recovery Environment":'tech.recovery.winreTitle',
  "REAgentC is available, but Phase 19 does not infer enabled/disabled WinRE state from localized console prose. Live state qualification remains pending on Windows.":'tech.recovery.winreUnverified',
  "The supported REAgentC executable was not found at the trusted System32 path; recovery state is unknown, not assumed unavailable.":'tech.recovery.winreMissing',
  "System Restore readiness":'tech.recovery.restoreTitle',
  "System Restore availability and restore-point creation success are treated as runtime recovery evidence; directory presence alone is not claimed as protection.":'tech.recovery.restoreEvidence',
  'Windows temporary files':'tech.cleanup.windowsTempTitle',
  'Temporary files older than 48 hours under the Windows temp root.':'tech.cleanup.windowsTempDesc',
  'Profile-specific temp files older than seven days. Because the service cannot infer that this is the interactive caller’s profile, this category requires explicit review.':'tech.cleanup.userTempDesc',
  'Profile-specific rebuildable Direct3D cache files older than 72 hours. This category requires explicit review.':'tech.cleanup.shaderDesc',
  'Windows Error Reporting archives':'tech.cleanup.werArchiveTitle',
  'Archived Windows Error Reporting files. Keep these when diagnosing crashes.':'tech.cleanup.werArchiveDesc',
  'Windows Error Reporting queue':'tech.cleanup.werQueueTitle',
  'Queued Windows Error Reporting files. Keep these when diagnosing crashes.':'tech.cleanup.werQueueDesc',
  'Windows minidumps':'tech.cleanup.minidumpsTitle',
  'Windows crash minidumps. Keep these when diagnosing BSODs.':'tech.cleanup.minidumpsDesc',
  'Windows memory dump':'tech.cleanup.memoryDumpTitle',
  'Full kernel memory dump. Keep it when diagnosing crashes.':'tech.cleanup.memoryDumpDesc',
  'Reviewed cleanup queued':'tech.cleanup.queued',
  'Revalidating allowlisted cleanup targets':'tech.cleanup.preflight',
  'Deleting reviewed candidates with final-path validation':'tech.cleanup.executing',
  'Verifying cleanup journal and reclaimed totals':'tech.cleanup.verifying',
  'Reviewed cleanup completed. Locked or changed files were left untouched.':'tech.cleanup.completed',
  'Cleanup stopped; no targets will be replayed automatically.':'tech.cleanup.stopped',
  'Cleanup stopped after deletion began. No deletion was replayed automatically.':'tech.cleanup.interruptedAfter',
  'Cleanup stopped before deletion began. No files were changed.':'tech.cleanup.interruptedBefore',
  'Service restart interrupted cleanup':'tech.cleanup.restartFailure',
  'Runs at user logon; no directly correlated boot-duration evidence was found by this inventory provider.':'tech.startup.logonEvidence',
  'Present in a Startup folder; no directly correlated duration evidence is asserted.':'tech.startup.folderEvidence',
  'Task has a boot/logon trigger; no undocumented duration fields are converted into a performance claim.':'tech.startup.taskEvidence',
  'Microsoft Windows or security-related scheduled tasks are protected.':'tech.startup.protectedTask',
  'Networking, storage, input, accessibility, or other protected service role.':'tech.startup.protectedRole',
  'NVMe SMART/Health log via IOCTL_STORAGE_QUERY_PROPERTY':'tech.storage.sourceNvme',
  'Direct NVMe SMART/Health log was not available for this device.':'tech.storage.sourceNvmeUnavailable',
  'ATA SMART attribute table via SMART_RCV_DRIVE_DATA; raw values are vendor-defined and are not converted into AetherCore health claims.':'tech.storage.sourceAta',
  'Direct ATA SMART attributes were not available through SMART_RCV_DRIVE_DATA; standardized Windows reliability counters remain authoritative when present.':'tech.storage.sourceAtaUnavailable',
  'Physical disk DeviceId was not reported.':'tech.storage.deviceIdMissing',
  'Windows reports the physical disk as unhealthy.':'tech.storage.unhealthy',
  'Windows reports a warning health state for this physical disk.':'tech.storage.warning',
  'NVMe SMART reports one or more media/data-integrity errors.':'tech.storage.mediaErrors',
  'The device-reported wear estimate has reached or exceeded its estimated wear limit.':'tech.storage.wearLimit',
  'Storage reliability evidence requires attention; back up important data before heavy write activity.':'tech.storage.summaryAction',
  'One or more reported storage reliability indicators deserve review.':'tech.storage.summaryAttention',
  'Windows/device-reported metrics that are available do not currently show a reliability warning.':'tech.storage.summaryNormal',
  'The device does not expose enough standardized reliability information for a health conclusion.':'tech.storage.summaryUnknown',
  'Windows Hardware Error Architecture recorded a hardware error.':'tech.event.wheaSummary',
  'WHEA logged a memory-related hardware error. This is hardware evidence, but it does not identify a specific DIMM without deeper decoding/testing.':'tech.event.wheaMemory',
  'WHEA logged a processor/cache-related hardware error. Treat this as evidence, not a complete root-cause attribution.':'tech.event.wheaProcessor',
  'WHEA logged PCI/PCIe-related hardware-error evidence.':'tech.event.wheaPcie',
  'WHEA logged a hardware error; the summarized event data is not sufficient to name a failed component with confidence.':'tech.event.wheaGeneric',
  'Windows recorded an unexpected shutdown or restart.':'tech.event.shutdownSummary',
  'Kernel-Power Event 41 confirms an unclean shutdown; by itself it does not identify why power was lost or the system crashed.':'tech.event.shutdownDetail',
  'Windows Error Reporting recorded a system crash/bugcheck.':'tech.event.werSummary',
  'The event is crash evidence. A precise driver/module attribution may require the matching dump plus symbols.':'tech.event.werDetail',
  'Relevant system diagnostic event.':'tech.event.genericSummary',
  'Event retained as supporting evidence.':'tech.event.genericDetail',
  'A Windows kernel dump is present with bugcheck header metadata. Full driver/module attribution requires symbol-assisted dump analysis.':'tech.crash.dumpWithHeader',
  'A minidump file is present; its Windows dump header was not recognized by the lightweight metadata parser.':'tech.crash.dumpMetadataOnly',
  'Back up important files immediately if errors or critical warnings are present.':'tech.card.backupNow',
  'Avoid unnecessary heavy write workloads until the device is checked.':'tech.card.avoidWrites',
  "Run the drive manufacturer's diagnostic utility and review firmware/support guidance.":'tech.card.vendorDiagnostics',
  'Replace the drive if reliability errors persist or increase.':'tech.card.replaceDrive',
  'Memory hardware-event evidence is unavailable':'tech.card.memoryUnavailableTitle',
  'The WHEA/Event Log collector did not complete, so AetherCore cannot state whether memory-related hardware errors were logged in this scan.':'tech.card.memoryUnavailableSummary',
  'Retry diagnostics after confirming Windows Event Log access.':'tech.card.retryEventLog',
  'Use Windows Memory Diagnostic if symptoms independently suggest memory instability.':'tech.card.memoryDiagnosticConditional',
  'Memory-related hardware errors were logged':'tech.card.memoryErrorsTitle',
  'Run Windows Memory Diagnostic at reboot for an offline test.':'tech.card.memoryDiagnosticReboot',
  'If diagnosing instability, remove overclock/XMP/EXPO variables before retesting.':'tech.card.removeOverclock',
  'If errors recur, test DIMMs individually and follow the system/vendor hardware service procedure.':'tech.card.testDimms',
  'No logged memory hardware errors were found':'tech.card.noMemoryErrorsTitle',
  'Use Windows Memory Diagnostic if you are troubleshooting suspected memory instability.':'tech.card.memoryDiagnostic',
  'Current memory pressure':'tech.card.memoryPressureTitle',
  'Close or inspect memory-heavy applications if performance is currently affected.':'tech.card.closeMemoryApps',
  'Review recent hardware/firmware/driver changes and vendor diagnostics before replacing components.':'tech.card.reviewHardwareChanges',
  'Bugcheck code not available from lightweight header parsing.':'tech.card.bugcheckUnavailable',
  'Recent Windows crash dump detected':'tech.card.recentCrashTitle',
  'Correlate the crash time with WHEA, driver and Windows Error Reporting events.':'tech.card.correlateCrash',
  'For module-level attribution, analyze the dump with matching Microsoft symbols; AetherCore does not infer a culprit from the filename alone.':'tech.card.symbolAnalysis',
  'Windows recorded a system crash':'tech.card.crashRecordedTitle',
  'Windows Error Reporting contains bugcheck/crash evidence, but no recent minidump metadata was available to the lightweight collector.':'tech.card.crashRecordedSummary',
  'Microsoft-Windows-WER-SystemErrorReporting event present.':'tech.card.werEvidence',
  'Check crash-dump configuration and correlate the event time with WHEA and recent driver changes.':'tech.card.crashConfig',
  'Do not assign a driver or hardware culprit without additional dump/event evidence.':'tech.card.noCulprit',
  'Unexpected shutdown recorded':'tech.card.shutdownTitle',
  'Kernel-Power evidence confirms an unclean shutdown, but no recent dump metadata was found by the lightweight collector.':'tech.card.shutdownSummary',
  'Kernel-Power Event 41 present.':'tech.card.kernelPowerEvidence',
  'Check power delivery, thermal stability, WHEA events, and crash-dump configuration before drawing a root-cause conclusion.':'tech.card.shutdownAction',
  'Windows launch-protected service.':'tech.startup.protectedLaunch',
  'Essential Windows service policy.':'tech.startup.protectedEssential',
  'Windows-hosted service.':'tech.startup.protectedWindows',
  'Security-related service policy.':'tech.startup.protectedSecurity',
  'Another service declares a dependency on this service.':'tech.startup.protectedDependency',
  'Shared/driver service types are not managed.':'tech.startup.protectedShared',
  'Automatic (delayed) service; exact boot impact is not inferred without direct telemetry.':'tech.startup.delayedEvidence',
  'Automatic service; exact boot impact is not inferred without direct telemetry.':'tech.startup.autoEvidence',
  'No directly correlated boot-duration evidence.':'tech.startup.noBootEvidence',
  'Keep enabled':'tech.startup.keepEnabled',
  'Review':'tech.startup.review',
  'Task Scheduler action':'tech.startup.taskSchedulerAction',
  'Authorized startup plan queued':'tech.startup.authorizedQueued',
  'Validating exact current startup state':'tech.startup.validatingCurrent',
  'Applying reviewed startup changes':'tech.startup.applyingReviewed',
  'Verifying exact post-change startup state':'tech.startup.verifyingCurrent',
  'All reviewed startup changes verified':'tech.startup.allVerified',
  'Startup change did not complete cleanly':'tech.startup.changeFailed',
  'Windows Update search completed with errors; applicability results may be incomplete.':'tech.driver.searchPartial',
  'Revalidating selected devices before any change.':'tech.driver.install.revalidateBefore',
  'Revalidating and downloading selected Windows-recommended drivers. No driver mutation has started.':'tech.driver.install.downloading',
  'Fresh restore point verified. Exporting currently bound OEM driver packages immediately before installation.':'tech.driver.install.backup',
  'Windows Update installation is authorized to start.':'tech.driver.install.authorized',
  'Checking device health and bound drivers.':'tech.driver.install.checkingHealth',
  'No currently bound driver package exists to export.':'tech.driver.install.noBoundDriver',
  'The bound driver is not an OEM Driver Store INF, so PnPUtil export is not applicable.':'tech.driver.install.nonOem',
  'PnP reports the device present without a problem code.':'tech.driver.install.pnpHealthy',
  'PnP reports a device problem after installation.':'tech.driver.install.pnpProblem',
  'Device is not present after installation.':'tech.driver.install.deviceAbsent',
  'Windows requires a restart before final verification can complete.':'tech.driver.install.rebootRequired',
  'All selected devices passed post-install PnP verification.':'tech.driver.install.allVerified',
  'One or more devices did not pass post-install verification.':'tech.driver.install.verificationFailed',
  'Service restarted after driver mutation; AetherCore will not replay installation automatically.':'tech.driver.install.interrupted',
  'Installation stopped before driver mutation.':'tech.driver.install.stoppedBefore',
  'Driver installation was interrupted':'tech.driver.install.interruptedSummary',
  'Installation was not replayed. Review the restore point and exported driver backup before further action.':'tech.driver.install.interruptedDetail',
  'Driver installation did not complete cleanly':'tech.driver.install.failedSummary',
  'A verified restore point and exported driver package evidence were preserved. No automatic rollback was attempted.':'tech.driver.install.failedDetail',
  'System Restore protection needs review':'tech.driver.install.restoreReviewSummary',
  'The restore point begin call succeeded, but Windows did not confirm the matching end/cancel call. Driver backup evidence remains preserved.':'tech.driver.install.restoreReviewDetail',
  'A device needs recovery review':'tech.driver.install.deviceReviewSummary',
  'The installation result or PnP health check failed. The restore point and driver backup evidence were preserved.':'tech.driver.install.deviceReviewDetail',
  'The operation stopped after the mutation barrier. AetherCore will not replay installation automatically.':'tech.driver.install.stoppedAfterBarrier',
  'The operation stopped before driver mutation. Any fresh restore point was cancelled when possible.':'tech.driver.install.stoppedBeforeBarrier',
  'Driver installation needs recovery review':'tech.driver.install.recoveryReviewSummary',
  'device inventory changed':'tech.driver.install.inventoryChanged',
  'Startup change interrupted':'tech.recovery.startupInterrupted',
  'AetherCore did not replay the mutation. Review Startup history before restoring or retrying.':'tech.recovery.startupInterruptedDetail',
  'Restart recovery observed original state; no mutation replayed.':'tech.recovery.startupNoChange',
  'Restart recovery observed the intended changed state; no mutation replayed.':'tech.recovery.startupApplied',
  'Restart recovery found ambiguous startup state; manual review required.':'tech.recovery.startupAmbiguous',
  'Startup operation was interrupted. AetherCore will not replay startup mutations automatically.':'tech.recovery.startupOperationInterrupted',
  'Exact original and target states durably recorded before mutation.':'tech.recovery.originalRecorded',
  'Native post-change verification matched the immutable target state.':'tech.recovery.changeVerified',
  'Change verified and reversible evidence retained.':'tech.recovery.evidenceRetained',
  'System repair needs review':'tech.recovery.repairReview',
  'No repair command was replayed automatically after restart. Review CBS/DISM logs and run a fresh assessment.':'tech.recovery.repairDetail',
  'Cleanup was interrupted after deletion began':'tech.recovery.cleanupInterrupted',
  'AetherCore did not replay deletion after restart. Run a fresh cleanup scan before any further action.':'tech.recovery.cleanupDetail',
};

export type LocalizedOwnedText = { text: string; localized: boolean };
export function localizeOwnedText(value: string, locale: Locale): LocalizedOwnedText {
  if (hasMessageKey(value)) return { text: td(value, locale), localized: true };
  if (!value || locale === 'en') return { text: value, localized: true };
  const exact = exactOwnedText[value];
  if (exact) return { text: td(exact, locale), localized: true };
  let m: RegExpMatchArray | null;
  if ((m = value.match(/^(\d+) uncorrected read error\(s\) were reported\.$/))) return { text: t('tech.storage.readErrors', locale, { count: m[1] }), localized: true };
  if ((m = value.match(/^(\d+) uncorrected write error\(s\) were reported\.$/))) return { text: t('tech.storage.writeErrors', locale, { count: m[1] }), localized: true };
  if ((m = value.match(/^NVMe SMART critical-warning flags are set \((0x[0-9A-Fa-f]+)\)\.$/))) return { text: t('tech.storage.nvmeCritical', locale, { flag: m[1] }), localized: true };
  if ((m = value.match(/^Current temperature \(([-\d.]+) °C\) is at or above the device\/Windows-reported maximum \(([-\d.]+) °C\)\.$/))) return { text: t('tech.storage.temperatureLimit', locale, { current:m[1], max:m[2] }), localized: true };
  if ((m = value.match(/^Windows reports a maximum (read|write|flush) latency above 10 seconds in the storage reliability counters\.$/))) {
    const key = ({read:'tech.storage.latencyRead',write:'tech.storage.latencyWrite',flush:'tech.storage.latencyFlush'} as const)[m[1] as 'read'|'write'|'flush'];
    return { text:td(key,locale), localized:true };
  }
  if ((m = value.match(/^Windows currently reports (\d+)% physical-memory load\. This is resource pressure, not a RAM hardware-health verdict\.$/))) return { text:t('tech.memory.pressure',locale,{load:m[1]}),localized:true };
  if ((m = value.match(/^(\d+) WHEA event\(s\) contained memory-related hardware-error evidence in the last (\d+) days\.$/))) return { text:t('tech.card.memoryErrorsSummary',locale,{count:m[1],days:m[2]}),localized:true };
  if ((m = value.match(/^No memory-related WHEA evidence was found in the last (\d+) days\. This does not prove RAM is fault-free\.$/))) return { text:t('tech.card.noMemoryErrorsSummary',locale,{days:m[1]}),localized:true };
  if ((m = value.match(/^Available physical memory: (\d+) of (\d+) bytes$/))) return { text:t('tech.card.availableMemory',locale,{available:m[1],total:m[2]}),localized:true };
  if ((m = value.match(/^Dump: (.+)$/))) return { text:t('tech.card.dumpEvidence',locale,{file:m[1]}),localized:true };
  if ((m = value.match(/^Bugcheck: (.+)$/))) return { text:t('tech.card.bugcheckEvidence',locale,{code:m[1]}),localized:true };
  if ((m = value.match(/^(\d+) WHEA event\(s\) were logged within ±10 minutes of the dump timestamp; this is correlation, not proof of causation\.$/))) return { text:t('tech.card.nearbyWhea',locale,{count:m[1]}),localized:true };
  if ((m = value.match(/^WHEA event (\d+): (.+)$/))) { const detail=localizeOwnedText(m[2],locale).text; return { text:t('tech.card.wheaEvidence',locale,{id:m[1],detail}),localized:true }; }
  if ((m = value.match(/^(.+) — storage reliability$/))) return { text:t('tech.card.storageTitle',locale,{device:m[1]}),localized:true };
  if ((m = value.match(/^(.+) temporary files$/))) return { text:t('tech.cleanup.userTempTitle',locale,{name:m[1]}),localized:true };
  if ((m = value.match(/^(.+) Direct3D shader cache$/))) return { text:t('tech.cleanup.shaderTitle',locale,{name:m[1]}),localized:true };
  if ((m = value.match(/^(.+) provider completed$/))) return { text:t('tech.cleanup.providerCompleted',locale,{provider:m[1]}),localized:true };
  if ((m = value.match(/^(.+); skipped (\d+) bytes that changed, were locked, or failed validation$/))) {
    const detail=localizeOwnedText(m[1],locale).text;
    return { text:t('tech.cleanup.itemResult',locale,{detail,bytes:m[2]}),localized:true };
  }
  if ((m = value.match(/^CHKDSK \/scan reported exit code (\d+)\. This phase does not run \/f or force an offline repair\. (.+)$/))) {
    return { text:t('tech.repair.chkdskAttention',locale,{code:m[1],detail:m[2]}),localized:true };
  }
  if (value === 'Diagnostic history persistence was unavailable; this scan remains visible in memory but was not added to durable history.') return { text:t('tech.warning.diagnosticPersistence',locale),localized:true };
  if (value === 'Diagnostic scan could not start because its worker thread was unavailable.') return { text:t('tech.warning.diagnosticSpawn',locale),localized:true };
  if ((m = value.match(/^Hardware telemetry: (.+)$/))) return { text:t('tech.warning.hardwarePrefix',locale,{detail:m[1]}),localized:true };
  if ((m = value.match(/^Crash diagnostics: (.+)$/))) return { text:t('tech.warning.crashPrefix',locale,{detail:m[1]}),localized:true };
  if ((m = value.match(/^Storage telemetry unavailable: (.+)$/))) return { text:t('tech.warning.storageUnavailable',locale,{detail:m[1]}),localized:true };
  if ((m = value.match(/^Windows Event Log evidence unavailable: (.+)$/))) return { text:t('tech.warning.eventLogUnavailable',locale,{detail:m[1]}),localized:true };
  if ((m = value.match(/^Windows Event Log collection unavailable: (.+)$/))) return { text:t('tech.warning.eventLogUnavailable',locale,{detail:m[1]}),localized:true };
  if ((m = value.match(/^Minidump metadata unavailable: (.+)$/))) return { text:t('tech.warning.minidumpUnavailable',locale,{detail:m[1]}),localized:true };
  if ((m = value.match(/^MSFT_StorageReliabilityCounter was unavailable: (.+)$/))) return { text:t('tech.storage.reliabilityUnavailable',locale,{detail:m[1]}),localized:true };
  if ((m = value.match(/^Service: (.+)$/))) return { text:t('tech.startup.serviceSource',locale,{name:m[1]}),localized:true };
  if ((m = value.match(/^Restart recovery could not inspect target: (.+)$/))) return { text:t('tech.startup.recoveryInspectFailed',locale,{detail:m[1]}),localized:true };
  if ((m = value.match(/^Restored by startup change (.+)\.$/))) return { text:t('tech.startup.restoredBy',locale,{changeId:m[1]}),localized:true };
  if ((m = value.match(/^(Disable|Restore): (.+)$/))) return { text:t('tech.startup.actionProgress',locale,{direction:localizeDirection(m[1],locale),name:m[2]}),localized:true };
  if ((m = value.match(/^(Disable|Restore) (RegistryRun|RegistryRunOnce|StartupFolder|ScheduledTask|Service)$/))) return { text:t('tech.startup.itemDetail',locale,{direction:localizeDirection(m[1],locale),kind:localizeKind(m[2],locale)}),localized:true };
  if ((m = value.match(/^PnP inventory failed: (.+)$/))) return { text:t('tech.driver.pnpInventoryFailed',locale,{detail:m[1]}),localized:true };
  if ((m = value.match(/^Windows Update discovery unavailable: (.+)$/))) return { text:t('tech.driver.discoveryUnavailable',locale,{detail:m[1]}),localized:true };
  if ((m = value.match(/^Windows Update search did not complete successfully: (.+)$/))) return { text:t('tech.driver.searchFailed',locale,{code:m[1]}),localized:true };
  if ((m = value.match(/^WUA item (\d+) could not be read: (.+)$/))) return { text:t('tech.driver.wuaItemRead',locale,{index:m[1],detail:m[2]}),localized:true };
  if ((m = value.match(/^WUA item (\d+) reported Type=Driver but did not expose IWindowsDriverUpdate$/))) return { text:t('tech.driver.wuaNoDriverInterface',locale,{index:m[1]}),localized:true };
  if ((m = value.match(/^WUA driver item (\d+) metadata was rejected: (.+)$/))) return { text:t('tech.driver.wuaDriverMetadata',locale,{index:m[1],detail:m[2]}),localized:true };
  if ((m = value.match(/^WUA item (\d+) entry (\d+) could not be read: (.+)$/))) return { text:t('tech.driver.wuaEntryRead',locale,{index:m[1],entry:m[2],detail:m[3]}),localized:true };
  if ((m = value.match(/^WUA item (\d+) entry (\d+) metadata was rejected: (.+)$/))) return { text:t('tech.driver.wuaEntryMetadata',locale,{index:m[1],entry:m[2],detail:m[3]}),localized:true };
  if ((m = value.match(/^WUA item (\d+) driver-entry count could not be read: (.+)$/))) return { text:t('tech.driver.wuaEntryCount',locale,{index:m[1],detail:m[2]}),localized:true };
  if ((m = value.match(/^WUA item (\d+) exposed IWindowsDriverUpdate4 but its entry collection could not be read: (.+)$/))) return { text:t('tech.driver.wuaEntryCollection',locale,{index:m[1],detail:m[2]}),localized:true };
  if ((m = value.match(/^WUA driver item (\d+) applicability metadata was rejected: (.+)$/))) return { text:t('tech.driver.wuaApplicability',locale,{index:m[1],detail:m[2]}),localized:true };
  if ((m = value.match(/^driver download failed: (.+)$/i))) return { text:t('tech.driver.downloadFailed',locale,{detail:m[1]}),localized:true };
  if ((m = value.match(/^download failed for selected update (\d+): (.+)$/i))) return { text:t('tech.driver.selectedDownloadFailed',locale,{index:m[1],detail:m[2]}),localized:true };
  if ((m = value.match(/^WUA pre-install revalidation was not complete: (.+)$/))) return { text:t('tech.driver.revalidationFailed',locale,{detail:m[1]}),localized:true };
  if ((m = value.match(/^Exported (\d+) files before mutation\.$/))) return { text:t('tech.driver.install.exportedFiles',locale,{count:m[1]}),localized:true };
  if ((m = value.match(/^Downloaded (\d+) of (\d+) bytes\.$/))) return { text:t('tech.driver.install.downloaded',locale,{downloaded:m[1],total:m[2]}),localized:true };
  if ((m = value.match(/^(.+) System Restore transaction close failed: (.+)$/))) { const base=localizeOwnedText(m[1],locale).text; return { text:`${base} ${t('tech.driver.install.restoreCloseFailed',locale,{detail:m[2]})}`,localized:true }; }
  if ((m = value.match(/^preflight rejected: (.+)$/))) { const detail=localizeOwnedText(m[1],locale).text; return { text:t('tech.driver.install.preflightPrefix',locale,{detail}),localized:true }; }
  if ((m = value.match(/^system protection failed: (.+)$/))) return { text:t('tech.driver.install.protectionPrefix',locale,{detail:m[1]}),localized:true };
  if ((m = value.match(/^Windows Update execution failed: (.+)$/))) { const detail=localizeOwnedText(m[1],locale).text; return { text:t('tech.driver.install.executionPrefix',locale,{detail}),localized:true }; }
  if ((m = value.match(/^post-install verification failed: (.+)$/))) return { text:t('tech.driver.install.verificationPrefix',locale,{detail:m[1]}),localized:true };
  if ((m = value.match(/^protected class (.+)$/))) return { text:t('tech.driver.install.protectedClass',locale,{className:m[1]}),localized:true };
  if ((m = value.match(/^device disappeared: (.+)$/))) return { text:t('tech.driver.install.deviceDisappeared',locale,{instanceId:m[1]}),localized:true };
  if ((m = value.match(/^device is no longer present: (.+)$/))) return { text:t('tech.driver.install.deviceNotPresent',locale,{instanceId:m[1]}),localized:true };
  if ((m = value.match(/^device class changed to protected class: (.+)$/))) return { text:t('tech.driver.install.classChangedProtected',locale,{className:m[1]}),localized:true };
  if ((m = value.match(/^device class changed since scan: (.+)$/))) return { text:t('tech.driver.install.classChanged',locale,{instanceId:m[1]}),localized:true };
  if ((m = value.match(/^matched hardware identity changed since scan: (.+)$/))) return { text:t('tech.driver.install.hardwareIdentityChanged',locale,{instanceId:m[1]}),localized:true };
  if ((m = value.match(/^bound driver changed since scan: (.+)$/))) return { text:t('tech.driver.install.boundDriverChanged',locale,{instanceId:m[1]}),localized:true };
  if ((m = value.match(/^device problem state changed since scan: (.+)$/))) return { text:t('tech.driver.install.problemStateChanged',locale,{instanceId:m[1]}),localized:true };
  if ((m = value.match(/^WUA did not return a per-update result for (.+) revision (\d+)$/))) return { text:t('tech.driver.install.wuaMissingResult',locale,{updateId:m[1],revision:m[2]}),localized:true };
  if ((m = value.match(/^(.+) event (\d+)$/))) return { text:t('tech.card.eventEvidence',locale,{provider:m[1],id:m[2]}),localized:true };
  return { text: value, localized: false };
}

export function localizeMemoryPressure(value: string, locale: Locale): string {
  const map: Record<string, MessageKey> = { Normal:'tech.memory.pressureNormal', Elevated:'tech.memory.pressureElevated', High:'tech.memory.pressureHigh' };
  return map[value] ? td(map[value],locale) : value;
}

export function localizeHealthStatus(value: string, locale: Locale): string {
  const map: Record<string, MessageKey> = { Healthy:'health.Healthy', Warning:'health.Warning', Unhealthy:'health.Unhealthy', Unknown:'health.Unknown' };
  return map[value] ? td(map[value], locale) : value;
}

export function localizeStartupScope(value: string, locale: Locale): string {
  const map: Record<string, MessageKey> = { 'Machine service':'tech.startup.scopeMachineService', 'Scheduled task':'tech.startup.scopeScheduledTask', 'Current user':'common.currentUser', 'All users':'common.allUsers' };
  return map[value] ? td(map[value], locale) : value;
}

export function localizePublisher(value: string, locale: Locale): string {
  const map: Record<string, MessageKey> = { 'Windows / security':'tech.startup.publisherWindowsSecurity', 'Third-party / unknown':'tech.startup.publisherThirdParty', 'Windows':'tech.startup.publisherWindows' };
  return map[value] ? td(map[value], locale) : value;
}

export function driverStateLabel(device: DriverDevice, locale: Locale): string {
  if (device.missingDriver) return t('drivers.state.missing',locale);
  if (device.hasProblem) return t('drivers.state.problem',locale,{code:device.problemCode});
  if (device.candidates.some((candidate) => candidate.firmwareManaged)) return t('drivers.state.firmware',locale);
  if (device.updateStatus === 'RecommendedUpdateAvailable') return t('drivers.state.update',locale);
  if (device.updateStatus === 'UpToDate') return t('drivers.state.upToDate',locale);
  if (device.updateStatus === 'NoUpdateFoundFromCheckedSources') return t('drivers.state.noConfirmedUpdate',locale);
  if (device.updateStatus === 'UpdateStatusUnknownOffline') return t('drivers.state.offlineUnknown',locale);
  if (device.updateStatus === 'ProviderUnavailable') return t('drivers.state.providerUnavailable',locale);
  if (device.updateStatus === 'UpdateStatusUnknown') return t('drivers.state.updateUnknown',locale);
  return t('drivers.state.none',locale);
}

export function driverTargetLabel(candidate: DriverCandidate, locale: Locale): string {
  return candidate.targetVersion || (candidate.driverDateIso ? t('drivers.target.offerDate',locale,{date:candidate.driverDateIso}) : t('drivers.target.offer',locale));
}
export function driverTargetEvidence(candidate: DriverCandidate, locale: Locale): string {
  if (candidate.targetVersion && candidate.targetVersionSource === 'TitleHeuristic') return t('drivers.target.parsedTitle',locale);
  if (candidate.targetVersion) return candidate.targetVersionSource || t('drivers.target.metadata',locale);
  return t('drivers.target.notExposed',locale);
}
export function localizeProviderFaultKind(kindCode: number, locale: Locale): string {
  const keys: Record<number, MessageKey> = {
    0: 'diagnostics.providerFaults.kind.unspecified',
    1: 'diagnostics.providerFaults.kind.timeout',
    2: 'diagnostics.providerFaults.kind.cancelled',
    3: 'diagnostics.providerFaults.kind.unavailable',
    4: 'diagnostics.providerFaults.kind.permissionDenied',
    5: 'diagnostics.providerFaults.kind.malformedResponse',
    6: 'diagnostics.providerFaults.kind.providerFailure',
    7: 'diagnostics.providerFaults.kind.io',
    8: 'diagnostics.providerFaults.kind.internal',
  };
  return td(keys[kindCode] ?? 'diagnostics.providerFaults.kind.unspecified', locale);
}

