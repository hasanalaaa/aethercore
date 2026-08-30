/**
 * Re-entrant activity accounting for UI work that may overlap.
 *
 * A boolean busy flag is incorrect when two asynchronous operations overlap: the first completion
 * can clear the flag while the second operation is still running. Each `enter` call returns an
 * idempotent release function so nested/overlapping work cannot underflow the counter.
 */
export class ActivityCounter {
  private count = 0;

  get active(): boolean { return this.count > 0; }
  get size(): number { return this.count; }

  enter(): () => void {
    this.count += 1;
    let released = false;
    return () => {
      if (released) return;
      released = true;
      this.count = Math.max(0, this.count - 1);
    };
  }
}
