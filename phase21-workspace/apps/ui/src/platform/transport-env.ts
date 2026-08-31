export function hasTauriInternals(value: unknown): boolean {
  return Boolean(value && typeof value === 'object' && '__TAURI_INTERNALS__' in value && (value as { __TAURI_INTERNALS__?: unknown }).__TAURI_INTERNALS__);
}
