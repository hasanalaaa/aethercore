import { get, writable } from 'svelte/store';
import type { DriverCandidate, DriverDevice, DriverFilter, DriverHub, DriverInstallStatus, Plan } from '../../lib/contracts';
import { currentShellState, setPage, runBusy } from '../../app/shell-state';
import { serviceInvoke } from '../../platform/service-client';
import { refreshServiceSnapshot } from '../../platform/snapshot';
import { patchStreamState, streamState } from '../../platform/stream-state';

export const scanStates: readonly string[] = ['InventoryScanning', 'UpdateSearching', 'Matching'];
export function scanStateIndex(state: string): number { return scanStates.findIndex((candidate) => candidate === state); }
export const driverFilters: DriverFilter[] = ['All', 'Updates', 'Problems', 'Missing', 'Display'];

type DriversUiState = {
  filter: DriverFilter;
  search: string;
  selected: Record<string, boolean>;
  expanded: Record<string, boolean>;
  lastSelectionScan: string;
  reviewOpen: boolean;
};

export const driversUi = writable<DriversUiState>({
  filter: 'All',
  search: '',
  selected: {},
  expanded: {},
  lastSelectionScan: '',
  reviewOpen: false,
});

function syncSelectionDefaults(hub: DriverHub): void {
  const ui = get(driversUi);
  if (!hub.scanId || hub.scanId === ui.lastSelectionScan || hub.state !== 'Ready') return;
  const selected: Record<string, boolean> = {};
  for (const device of hub.devices) {
    for (const candidate of device.candidates) selected[candidate.candidateId] = candidate.selectable && candidate.selectedByDefault;
  }
  driversUi.update((state) => ({ ...state, selected, lastSelectionScan: hub.scanId }));
}

streamState.subscribe((state) => syncSelectionDefaults(state.hub));

export function setDriverFilter(filter: DriverFilter): void { driversUi.update((state) => ({ ...state, filter })); }
export function setDriverSearch(search: string): void { driversUi.update((state) => ({ ...state, search })); }
export function toggleCandidate(candidateId: string, checked: boolean): void {
  driversUi.update((state) => ({ ...state, selected: { ...state.selected, [candidateId]: checked } }));
}

export function selectAllRecommended(): void {
  const { hub } = get(streamState);
  const selected: Record<string, boolean> = {};
  for (const device of hub.devices) {
    for (const candidate of device.candidates) {
      selected[candidate.candidateId] = candidate.selectable && candidate.recommendationState === 'Recommended';
    }
  }
  driversUi.update((state) => ({ ...state, selected }));
}

export function clearDriverSelection(): void {
  const { hub } = get(streamState);
  const selected: Record<string, boolean> = {};
  for (const device of hub.devices) for (const candidate of device.candidates) selected[candidate.candidateId] = false;
  driversUi.update((state) => ({ ...state, selected }));
}

export function recommendedSelectableCount(): number {
  return get(streamState).hub.devices.reduce((sum, device) => sum + device.candidates.filter((candidate) => candidate.selectable && candidate.recommendationState === 'Recommended').length, 0);
}

export function toggleExpanded(instanceId: string): void {
  driversUi.update((state) => ({ ...state, expanded: { ...state.expanded, [instanceId]: !state.expanded[instanceId] } }));
}
export function closeDriverReview(): void { driversUi.update((state) => ({ ...state, reviewOpen: false })); }

export function selectedCandidates(): DriverCandidate[] {
  const { hub } = get(streamState);
  const { selected } = get(driversUi);
  const candidates: DriverCandidate[] = [];
  for (const device of hub.devices) {
    for (const candidate of device.candidates) if (candidate.selectable && selected[candidate.candidateId]) candidates.push(candidate);
  }
  return candidates;
}

export function selectedUpdates(): DriverCandidate[] {
  const unique = new Map<string, DriverCandidate>();
  for (const candidate of selectedCandidates()) if (candidate.updateId) unique.set(`${candidate.updateId}:${candidate.revision}`, candidate);
  return [...unique.values()];
}

export function filteredDevices(): DriverDevice[] {
  const { hub } = get(streamState);
  const { filter, search } = get(driversUi);
  const query = search.trim().toLowerCase();
  return hub.devices.filter((device) => {
    const filterMatch = filter === 'All'
      || (filter === 'Updates' && device.candidates.some((candidate) => candidate.recommendationState === 'Recommended' || candidate.recommendationState === 'Optional'))
      || (filter === 'Problems' && device.hasProblem)
      || (filter === 'Missing' && device.missingDriver)
      || (filter === 'Display' && (device.className.toLowerCase() === 'display' || device.displayManaged));
    if (!filterMatch) return false;
    if (!query) return true;
    return [device.displayName, device.manufacturer, device.className, device.instanceId, ...device.hardwareIds]
      .some((value) => value.toLowerCase().includes(query));
  });
}

export async function startDriverScan(): Promise<void> {
  setPage('drivers');
  await runBusy(async () => {
    const hub = await serviceInvoke<DriverHub>('start_driver_scan');
    patchStreamState({ hub });
    driversUi.set({ filter: get(driversUi).filter, search: get(driversUi).search, selected: {}, expanded: {}, lastSelectionScan: '', reviewOpen: false });
  });
}

export async function setDriverCandidatePolicy(candidateId: string, policy: 'IgnoreExactVersion' | 'RemindLater' | 'IgnoreOptional'): Promise<void> {
  const { hub } = get(streamState);
  if (hub.state !== 'Ready' || !hub.scanId) return;
  await runBusy(async () => {
    const refreshed = await serviceInvoke<DriverHub>('set_driver_candidate_policy', {
      scanId: hub.scanId,
      inventoryEpoch: hub.inventoryEpoch,
      candidateId,
      policy,
    });
    patchStreamState({ hub: refreshed });
    driversUi.update((state) => ({ ...state, selected: { ...state.selected, [candidateId]: false } }));
  });
}

export async function openGpuSupport(vendor: string): Promise<void> {
  await runBusy(() => serviceInvoke<void>('open_gpu_vendor_support', { vendor }));
}

export async function reviewDriverInstall(): Promise<void> {
  const { hub } = get(streamState);
  const chosen = selectedCandidates();
  if (hub.state !== 'Ready' || !chosen.length) return;
  await runBusy(async () => {
    const installPlan = await serviceInvoke<Plan>('create_driver_install_plan', {
      scanId: hub.scanId,
      inventoryEpoch: hub.inventoryEpoch,
      candidateIds: chosen.map((candidate) => candidate.candidateId),
    });
    patchStreamState({ installPlan });
    driversUi.update((state) => ({ ...state, reviewOpen: true }));
    await refreshServiceSnapshot();
  });
}

export async function authorizeDriverPlan(): Promise<void> {
  const { installPlan } = get(streamState);
  if (!installPlan) return;
  await runBusy(async () => {
    await serviceInvoke<void>('approve_plan_with_uac', { planId: installPlan.id, locale: currentShellState().locale });
    await refreshServiceSnapshot();
  });
}

export async function startDriverInstall(): Promise<void> {
  const { installPlan } = get(streamState);
  if (!installPlan) return;
  await runBusy(async () => {
    const installStatus = await serviceInvoke<DriverInstallStatus>('start_driver_install', { planId: installPlan.id });
    patchStreamState({ installStatus });
    closeDriverReview();
    await refreshServiceSnapshot();
  });
}

export async function authorizeAndInstall(): Promise<void> {
  const { installPlan } = get(streamState);
  if (!installPlan) return;
  await runBusy(async () => {
    await serviceInvoke<void>('approve_plan_with_uac', { planId: installPlan.id, locale: currentShellState().locale });
    await refreshServiceSnapshot();
    const installStatus = await serviceInvoke<DriverInstallStatus>('start_driver_install', { planId: installPlan.id });
    patchStreamState({ installStatus });
    closeDriverReview();
    await refreshServiceSnapshot();
  });
}

export function installActive(): boolean {
  const status = get(streamState).installStatus;
  return !!status && !['Completed', 'Failed'].includes(status.planState);
}

export function installTerminal(): boolean {
  const status = get(streamState).installStatus;
  return !!status && ['Completed', 'Failed'].includes(status.planState);
}
