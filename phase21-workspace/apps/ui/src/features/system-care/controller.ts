import { get, writable } from 'svelte/store';
import { runBusy, shellState } from '../../app/shell-state';
import type { SupportBundlePreview, SupportExportResult, UpdateSnapshot } from '../../lib/contracts';
import { serviceInvoke } from '../../platform/service-client';
import { patchStreamState, streamState } from '../../platform/stream-state';

type SystemCareUi = { channel: 1 | 2; supportPreview: SupportBundlePreview | null; exportResult: SupportExportResult | null };
export const systemCareUi = writable<SystemCareUi>({ channel: 1, supportPreview: null, exportResult: null });

export async function checkUpdates(): Promise<void> {
  const channel = get(systemCareUi).channel;
  await runBusy(async () => {
    const updateSnapshot = await serviceInvoke<UpdateSnapshot>('check_for_updates', { channel });
    patchStreamState({ updateSnapshot });
  });
}
export function setUpdateChannel(channel: 1 | 2): void { systemCareUi.update((s) => ({ ...s, channel })); }
export async function stageLatestUpdate(): Promise<void> {
  const release = get(streamState).updateSnapshot.latestRelease;
  if (!release) return;
  await runBusy(async () => {
    const updateSnapshot = await serviceInvoke<UpdateSnapshot>('stage_update', { releaseId: release.releaseId });
    patchStreamState({ updateSnapshot });
  });
}
export async function installStagedUpdate(): Promise<void> {
  const release = get(streamState).updateSnapshot.stagedRelease;
  if (!release) return;
  await runBusy(() => serviceInvoke<void>('install_staged_update', { releaseId: release.releaseId, locale: get(shellState).locale }));
}
export async function previewSupportBundle(): Promise<void> {
  await runBusy(async () => {
    const supportPreview = await serviceInvoke<SupportBundlePreview>('create_support_bundle_preview', {
      includeHardware: true,
      includeCrashMetadata: true,
      includeOperationHistory: true,
      includeSchedulerActivity: true,
    });
    systemCareUi.update((s) => ({ ...s, supportPreview, exportResult: null }));
  });
}
export async function exportSupportBundle(): Promise<void> {
  const preview = get(systemCareUi).supportPreview;
  if (!preview) return;
  await runBusy(async () => {
    const exportResult = await serviceInvoke<SupportExportResult>('export_support_bundle', { previewId: preview.previewId });
    systemCareUi.update((s) => ({ ...s, exportResult }));
  });
}
