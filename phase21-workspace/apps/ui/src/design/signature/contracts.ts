/**
 * The types behind the four signature elements.
 *
 * These are not conveniences. Each one exists so that a product invariant is
 * enforced by the compiler rather than by everyone remembering it:
 *
 *  - a policy refusal cannot be rendered without naming the rule that refused;
 *  - an insight cannot be rendered without the observation it cites;
 *  - a measurement that was never taken cannot be shown as a number.
 */

/**
 * The observation a displayed claim rests on.
 *
 * Both fields are required, and `EvidenceChip` takes an `Evidence` rather than an
 * `Evidence | undefined`. That is the whole mechanism: uncitable insights are
 * dropped before display, so the UI must have no way to render one. Widening
 * either field to optional re-opens that hole.
 */
export type Evidence = {
  /** Short citation for the chip face, e.g. `WER · 2 events`. Localized prose. */
  cite: string;
  /** The raw observation itself: provider, sampling, window, counts. Monospace. */
  raw: string;
};

/** An item that has passed the citation gate. `citedOnly` is the only producer. */
export type Cited<T> = T & { readonly evidence: Evidence };

/**
 * Drops every item that cannot cite an observation.
 *
 * Call this at the boundary where service data becomes view data. Anything that
 * survives can be rendered; anything that does not was never displayable. The
 * count of what it dropped is returned so a screen can say so honestly instead
 * of quietly showing a shorter list.
 */
export function citedOnly<T>(
  items: readonly T[],
  toEvidence: (item: T) => Evidence | null,
): { cited: Cited<T>[]; dropped: number } {
  const cited: Cited<T>[] = [];
  let dropped = 0;
  for (const item of items) {
    const evidence = toEvidence(item);
    if (evidence && evidence.cite && evidence.raw) cited.push({ ...item, evidence });
    else dropped += 1;
  }
  return { cited, dropped };
}

/**
 * An operation the system refused on purpose.
 *
 * `rule` is required. A refusal that cannot name the rule behind it is
 * indistinguishable from a failure, and the whole point of this element is that
 * the two never look alike.
 */
export type PolicyDenial = {
  /** What was refused, in the user's language. */
  label: string;
  /** The rule id that refused it, e.g. `NET-NO-EGRESS`. Monospace, LTR-isolated. */
  rule: string;
};

/** One channel an honest empty state is waiting on. Shows `—`, never `0`. */
export type EmptyChannel = {
  label: string;
  /**
   * The reading, once there is one. Absent means nothing has been collected, and
   * the channel renders an em dash. `0` is a real measurement and displays as 0;
   * that distinction is the entire reason this is `number | undefined` and not a
   * number defaulting to zero.
   */
  value?: number | string;
};
