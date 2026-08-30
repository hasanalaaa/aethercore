import { get } from 'svelte/store';
import type { DiagnosticsSnapshot } from '../../lib/contracts';
import { runBusy, setPage } from '../../app/shell-state';
import { serviceInvoke } from '../../platform/service-client';
import { patchStreamState, streamState } from '../../platform/stream-state';

export async function startDiagnosticsScan(page: 'hardware' | 'crash' = 'hardware'): Promise<void> {
  setPage(page);
  await runBusy(async () => {
    const diagnostics = await serviceInvoke<DiagnosticsSnapshot>('start_diagnostics_scan');
    patchStreamState({ diagnostics });
  });
}

export function diagnosticRunning(): boolean { return get(streamState).diagnostics.state === 'Collecting'; }
export function storageActionCount(): number { return get(streamState).diagnostics.storage.filter((device) => device.severity === 'ActionRequired').length; }
export function memoryEvents() { return get(streamState).diagnostics.events.filter((event) => event.category === 'MemoryHardwareEvidence'); }
export function eventTone(severity: string): 'warn' | 'ok' { return severity === 'ActionRequired' || severity === 'Attention' ? 'warn' : 'ok'; }
