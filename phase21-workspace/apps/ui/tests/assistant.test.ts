// P86-01: text the model streams before the citation gate runs is not kept, so no screen or screen
// reader can show it; only an ANSWERED turn keeps its text.
// Run: node --experimental-strip-types --import ./tests/resolve-ts.mjs --test tests/*.test.ts
import { test } from 'node:test';
import assert from 'node:assert/strict';
import {
  TURN_ANSWERED,
  TURN_CANCELLED,
  TURN_FAULTED,
  TURN_STREAMING,
  settleTurn,
} from '../src/features/assistant/settle.ts';
import type { AssistantState } from '../src/features/assistant/controller.ts';
import type { AssistantTurn } from '../src/lib/contracts.ts';

const hostile = 'The plan completed [E1]. The disk will fail tomorrow.';
const empty: AssistantState = { transcript: [], inFlight: '', pack: [], engineLabel: '', packRead: false };

const turn = (state: number, answer: string): AssistantTurn => ({
  turnId: 'turn-a',
  schemaVersion: 1,
  state,
  answer,
  citations: [],
  engineLabel: 'localModel',
  refusal: 0,
  faultKey: state === TURN_FAULTED ? 'assistant.fault.deadlineExceeded' : '',
  tokensEmitted: 12,
  pack: [],
});

const settled = (...turns: AssistantTurn[]): AssistantTurn => {
  const state = turns.reduce(settleTurn, empty);
  const entry = state.transcript.at(-1);
  assert.equal(entry?.kind, 'turn');
  return (entry as { turn: AssistantTurn }).turn;
};

test('a streamed frame keeps its progress but not its text', () => {
  const kept = settled(turn(TURN_STREAMING, hostile));
  assert.equal(kept.answer, '');
  assert.equal(kept.tokensEmitted, 12);
  assert.equal(settleTurn(empty, turn(TURN_STREAMING, hostile)).inFlight, 'turn-a');
});

test('a turn that ends cancelled or faulted after streaming keeps no text', () => {
  for (const state of [TURN_CANCELLED, TURN_FAULTED]) {
    const kept = settled(turn(TURN_STREAMING, hostile), turn(state, 'deadline exceeded after 40 tokens'));
    assert.equal(kept.answer, '', String(state));
    assert.equal(kept.state, state);
  }
});

test('an answered turn keeps the text the gate admitted, in either language', () => {
  for (const answer of ['A plan completed [E1].', 'اكتملت خطة صيانة [E1].']) {
    assert.equal(settled(turn(TURN_STREAMING, hostile), turn(TURN_ANSWERED, answer)).answer, answer);
  }
});
