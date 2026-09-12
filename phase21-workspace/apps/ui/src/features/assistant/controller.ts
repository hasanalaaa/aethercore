/**
 * Phase 57 — the assistant drawer's state.
 *
 * The service side landed in P56 and the engine generates; there was nothing to
 * type into. This is that half.
 *
 * Three rules, all of them from `docs/phase56/DIRECTION.md` Part 2.A, and each
 * one is a thing this file is written to make impossible rather than a thing to
 * remember:
 *
 * 1. **Streamed text is provisional.** The citation gate runs on a COMPLETE
 *    answer, so `STREAMING.answer` is not an answer. `isProvisional` is derived
 *    from the state alone, never from whether text is present, and the drawer
 *    renders it under its own treatment.
 * 2. **An ungrounded turn is a refusal, not dim prose.** The service already
 *    discards uncited text and terminates REFUSED; nothing here may resurrect
 *    it, which is why the transcript keeps the terminal envelope and not the
 *    frames that preceded it.
 * 3. **The transcript is session state.** It lives here, in the renderer, and
 *    dies with the window — exactly as insights do. No persistence, no replay.
 */
import { get, writable } from 'svelte/store';
import type { AssistantEvidenceRef, AssistantPackResponse, AssistantTurn } from '../../lib/contracts';
import { serviceInvoke } from '../../platform/service-client';
import { streamState } from '../../platform/stream-state';

/** `AssistantTurnState` from `assistant.proto`. Prost sends the ordinal. */
export const TURN_STREAMING = 1;
export const TURN_ANSWERED = 2;
export const TURN_REFUSED = 3;
export const TURN_FAULTED = 4;
export const TURN_CANCELLED = 5;

/** `AssistantRefusalReason` from `assistant.proto`. */
export const REFUSAL_NO_EVIDENCE = 1;
export const REFUSAL_NOT_COVERED = 2;
export const REFUSAL_MUTATION_ACTIVE = 3;
export const REFUSAL_BUSY = 4;

/** Service-side bound, restated so the input can stop the user at the same number. */
export const MAX_QUESTION_CHARS = 2000;

/** One line of the transcript: what was asked, or what came back. */
export type AssistantEntry =
  | { kind: 'question'; id: string; text: string }
  | { kind: 'turn'; id: string; turn: AssistantTurn };

export type AssistantState = {
  /** Oldest first. Question and turn alternate; a turn always follows its question. */
  transcript: readonly AssistantEntry[];
  /** The turn id currently generating, or `''`. Exactly one at a time, per the wire. */
  inFlight: string;
  /** What the assistant may draw on right now, read without starting a turn. */
  pack: readonly AssistantEvidenceRef[];
  /** localModel | ruleFallback | disabled | '' before the pack has been read. */
  engineLabel: string;
  /** Whether `pack` has been read at all — `[]` before and `[]` after differ. */
  packRead: boolean;
};

export const assistantState = writable<AssistantState>({
  transcript: [],
  inFlight: '',
  pack: [],
  engineLabel: '',
  packRead: false,
});

/** Streaming text is never an answer. Derived from state, never from the text. */
export function isProvisional(turn: AssistantTurn): boolean {
  return turn.state === TURN_STREAMING;
}

/**
 * Which surfaces the pack holds, and how many rows of each.
 *
 * In the order the service composed them, not by size: `compose_evidence_pack`
 * appends maintenance history and then timeline patterns, so this reads the same
 * way on every open instead of reordering itself as rows arrive.
 */
export function packCounts(pack: readonly AssistantEvidenceRef[]): { surface: string; count: number }[] {
  const counts = new Map<string, number>();
  for (const item of pack) counts.set(item.surface, (counts.get(item.surface) ?? 0) + 1);
  return [...counts.entries()].map(([surface, count]) => ({ surface, count }));
}

/**
 * Splits an answer into text and the `[En]` markers the grounding gate left in it.
 *
 * The markers stay in the text on purpose (`intelligence-core/src/assistant.rs`:
 * "a marker that resolves STAYS in the text"), so the reader can see which
 * clause rests on which observation. A marker resolves against the turn's own
 * pack by position — `[E1]` is `pack[0]` — and one that does not resolve is
 * dropped rather than shown as a dead reference.
 */
export type AnswerSegment =
  | { kind: 'text'; text: string }
  | { kind: 'cite'; index: number; evidence: AssistantEvidenceRef };

export function segmentAnswer(turn: AssistantTurn): AnswerSegment[] {
  const segments: AnswerSegment[] = [];
  const pattern = /\[E(\d+)\]/g;
  let cursor = 0;
  for (let match = pattern.exec(turn.answer); match; match = pattern.exec(turn.answer)) {
    const evidence = turn.pack[Number(match[1]) - 1];
    if (match.index > cursor) segments.push({ kind: 'text', text: turn.answer.slice(cursor, match.index) });
    if (evidence) segments.push({ kind: 'cite', index: Number(match[1]), evidence });
    cursor = match.index + match[0].length;
  }
  if (cursor < turn.answer.length) segments.push({ kind: 'text', text: turn.answer.slice(cursor) });
  return segments;
}

/**
 * A client-assigned handle. The client needs it before the first event arrives,
 * which is why the CLIENT assigns it; the service validates the shape and uses
 * it as a map key, so it stays inside `[A-Za-z0-9_-]`.
 */
function newTurnId(): string {
  const random = crypto.getRandomValues(new Uint8Array(8));
  return `turn-${Array.from(random, (byte) => byte.toString(16).padStart(2, '0')).join('')}`;
}

function replaceTurn(state: AssistantState, turn: AssistantTurn): AssistantState {
  let found = false;
  const transcript = state.transcript.map((entry) => {
    if (entry.kind !== 'turn' || entry.turn.turnId !== turn.turnId) return entry;
    found = true;
    return { ...entry, turn };
  });
  return {
    ...state,
    transcript: found ? transcript : [...transcript, { kind: 'turn', id: turn.turnId, turn }],
    inFlight: turn.state === TURN_STREAMING ? turn.turnId : state.inFlight === turn.turnId ? '' : state.inFlight,
    // Every turn carries what it was allowed to draw on, so the empty state's
    // counts stay current without a second read.
    pack: turn.pack.length ? turn.pack : state.pack,
    engineLabel: turn.engineLabel || state.engineLabel,
    packRead: state.packRead || turn.pack.length > 0,
  };
}

/**
 * Applies a streamed envelope.
 *
 * Late frames are ignored: a STREAMING frame that arrives after the terminal one
 * would otherwise reopen a finished turn and put provisional text back on the
 * screen. The kernel stream is ordered, so this guards against nothing but a
 * reordering bug — which is exactly the kind that puts an uncited claim on
 * screen, and is therefore worth one line.
 */
export function applyAssistantTurn(turn: AssistantTurn): void {
  assistantState.update((state) => {
    const existing = state.transcript.find(
      (entry): entry is Extract<AssistantEntry, { kind: 'turn' }> =>
        entry.kind === 'turn' && entry.turn.turnId === turn.turnId,
    );
    if (existing && existing.turn.state !== TURN_STREAMING && turn.state === TURN_STREAMING) return state;
    return replaceTurn(state, turn);
  });
}

let streamBound = false;

/** Subscribes the transcript to the kernel stream. Idempotent; the shell calls it once. */
export function bindAssistantStream(): void {
  if (streamBound) return;
  streamBound = true;
  let last: AssistantTurn | null = null;
  streamState.subscribe((state) => {
    if (!state.assistantTurn || state.assistantTurn === last) return;
    last = state.assistantTurn;
    applyAssistantTurn(last);
  });
}

/**
 * Reads what the assistant may draw on, without starting a turn.
 *
 * The drawer's empty state counts rows rather than listing capabilities, and it
 * has to do that before the user has asked anything — which is why this verb
 * exists (`GetAssistantPack`, P57) rather than the drawer waiting for a turn.
 */
export async function loadAssistantPack(): Promise<void> {
  try {
    const response = await serviceInvoke<AssistantPackResponse>('get_assistant_pack');
    assistantState.update((state) => ({
      ...state,
      pack: response.pack ?? [],
      engineLabel: response.engineLabel ?? '',
      packRead: true,
    }));
  } catch {
    // Offline is a normal early state, not an error dialog. `packRead` stays
    // false so the drawer says "not collected yet" rather than "zero rows" —
    // the two are different claims and only one of them was measured.
  }
}

/**
 * Asks a question.
 *
 * Returns immediately: the reply is either a terminal turn decided without the
 * model (no evidence, a mutation holding the lease, no model) or a STREAMING
 * turn whose frames arrive on the kernel stream.
 */
export async function askAssistant(question: string): Promise<void> {
  const text = question.trim();
  if (!text || get(assistantState).inFlight) return;
  const turnId = newTurnId();
  assistantState.update((state) => ({
    ...state,
    transcript: [...state.transcript, { kind: 'question', id: turnId, text }],
    inFlight: turnId,
  }));
  try {
    const turn = await serviceInvoke<AssistantTurn>('ask_assistant', { turnId, question: text });
    assistantState.update((state) => replaceTurn(state, turn));
  } catch (error) {
    // A transport failure is a FAULT, not an empty answer and not a silent
    // degrade: the drawer must show a declared reason. `engineLabel` keeps
    // reporting what the engine IS, which is what the last read said.
    assistantState.update((state) =>
      replaceTurn(state, {
        turnId,
        schemaVersion: 1,
        state: TURN_FAULTED,
        answer: String(error),
        citations: [],
        engineLabel: state.engineLabel,
        refusal: 0,
        faultKey: 'assistant.fault.transport',
        tokensEmitted: 0,
        pack: [...state.pack],
      }),
    );
  }
}

/**
 * Cancels the turn in flight.
 *
 * Its own verb: `ask_assistant` has already returned by the time Escape is
 * pressed, so cancelling the REQUEST would cancel nothing. The flag is read
 * between tokens and the turn terminates CANCELLED with whatever it had — the
 * terminal envelope on the stream settles the transcript, not this call.
 */
export async function cancelAssistantTurn(): Promise<boolean> {
  const turnId = get(assistantState).inFlight;
  if (!turnId) return false;
  try {
    await serviceInvoke<AssistantTurn>('cancel_assistant_turn', { turnId });
  } catch {
    // The terminal envelope still settles the turn; a failed cancel is not a
    // second fault to report.
  }
  return true;
}
