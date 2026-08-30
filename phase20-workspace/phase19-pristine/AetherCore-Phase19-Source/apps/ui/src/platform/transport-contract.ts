export type UiTransportUnlisten = () => void;
export type UiTransportEvent<T> = { payload: T };

/** Narrow renderer transport boundary used by the production Tauri adapter and deterministic E2E tests. */
export interface UiTransport {
  invoke<T>(command: string, args?: Record<string, unknown>): Promise<T>;
  listen<T>(event: string, handler: (event: UiTransportEvent<T>) => void): Promise<UiTransportUnlisten>;
}
