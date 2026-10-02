import type { TimelineResponse } from '../../lib/contracts';

/** Admit only replies for this renderer session and its pinned, bounded history. */
export class TimelinePaging {
  state = { page: null as TimelineResponse | null, loading: false, readFailed: false, reloadRequired: false, newActivity: false, selectedEventSourceId: '' };
  private generation = 0;
  private session = '';
  private pinned = false;
  private invalidate(): void {
    const reload = this.pinned || this.state.loading;
    ++this.generation; this.pinned = false;
    this.state = { page: null, loading: false, readFailed: false, reloadRequired: reload, newActivity: false, selectedEventSourceId: '' };
  }
  observeSession(key: string): boolean {
    if (this.session === key) return false;
    this.session = key; this.invalidate(); return true;
  }
  observeLivePage(page: TimelineResponse | null): void {
    if (!page) { this.invalidate(); return; }
    if (this.pinned) {
      if (page.digestSha256 !== this.state.page?.digestSha256) this.state.newActivity = true;
    } else if (!this.state.loading && !this.state.reloadRequired) {
      this.state.page = page;
    }
  }
  begin(older: boolean): { ticket: number; args: Record<string, unknown> } | null {
    const page = this.state.page;
    if (older && (!page?.hasMore || this.state.loading || this.state.reloadRequired)) return null;
    this.state.loading = true; this.state.readFailed = false;
    return { ticket: ++this.generation, args: { pageSize: 100, beforeSequence: older ? page!.nextBeforeSequence : 0,
      snapshotCursor: older ? page!.nextSnapshotCursor : '' } };
  }
  accept(ticket: number, page: TimelineResponse, older: boolean): boolean {
    if (ticket !== this.generation) return false;
    if (page.reloadRequired || (older && page.digestSha256 !== this.state.page?.digestSha256)) {
      this.invalidate(); this.state.reloadRequired = true; return true;
    }
    this.state.page = older && this.state.page ? { ...page, entries: [...page.entries, ...this.state.page.entries] } : page;
    this.select(this.state.selectedEventSourceId);
    this.state.loading = false; this.state.reloadRequired = false; this.pinned = true;
    if (!older) this.state.newActivity = false;
    return true;
  }
  select(sourceId: string): boolean {
    const loaded = this.state.page?.entries.some((entry) => entry.sourceId === sourceId) ?? false;
    this.state.selectedEventSourceId = loaded ? sourceId : '';
    return loaded;
  }
  fail(ticket: number): void {
    if (ticket !== this.generation) return;
    this.state.loading = false; this.state.readFailed = true;
  }
}
