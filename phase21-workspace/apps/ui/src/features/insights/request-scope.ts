/** A late reply may only update the exact locale/evidence view that requested it. */
export class RequestScope {
  private generation = 0;
  invalidate(): void { this.generation += 1; }
  begin(): number { return this.generation; }
  current(ticket: number): boolean { return ticket === this.generation; }
}
