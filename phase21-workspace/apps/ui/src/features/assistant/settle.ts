/**
 * What the transcript keeps of each turn the service sends. Pure, so it is tested without a
 * renderer or a service (`tests/assistant.test.ts`).
 */
import type { AssistantTurn } from '../../lib/contracts';
import type { AssistantEntry, AssistantState } from './controller';

/** `AssistantTurnState` from `assistant.proto`. Prost sends the ordinal. */
export const TURN_STREAMING = 1;
export const TURN_ANSWERED = 2;
export const TURN_REFUSED = 3;
export const TURN_FAULTED = 4;
export const TURN_CANCELLED = 5;

/**
 * Only an ANSWERED turn has passed the citation gate, so only its text is kept (P86-01). A
 * streamed frame carries the model's raw text as it grows, uncited claims included, and a fault
 * carries a service diagnostic in the same field; text the transcript never holds is text no
 * screen or screen reader can reach, whatever a later edit to the drawer does.
 */
export function replaceTurn(state: AssistantState, received: AssistantTurn): AssistantState {
  const turn = received.state === TURN_ANSWERED ? received : { ...received, answer: '' };
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
 * A streamed envelope, applied.
 *
 * Late frames are ignored: a STREAMING frame that arrives after the terminal one
 * would otherwise reopen a finished turn and put provisional text back on the
 * screen. The kernel stream is ordered, so this guards against nothing but a
 * reordering bug — which is exactly the kind that puts an uncited claim on
 * screen, and is therefore worth one line.
 */
export function settleTurn(state: AssistantState, turn: AssistantTurn): AssistantState {
  const existing = state.transcript.find(
    (entry): entry is Extract<AssistantEntry, { kind: 'turn' }> =>
      entry.kind === 'turn' && entry.turn.turnId === turn.turnId,
  );
  if (existing && existing.turn.state !== TURN_STREAMING && turn.state === TURN_STREAMING) return state;
  return replaceTurn(state, turn);
}
