// P86-01: text the model streams before the citation gate runs is not kept, so no screen or screen
// reader can show it; only an ANSWERED turn keeps its text.
// Run: node --experimental-strip-types --import ./tests/resolve-ts.mjs --test tests/*.test.ts
import { test } from 'node:test';
import assert from 'node:assert/strict';
import { get } from 'svelte/store';
import {
  TURN_ANSWERED,
  TURN_CANCELLED,
  TURN_FAULTED,
  TURN_STREAMING,
  applyAssistantTurn,
  assistantState,
} from '../src/features/assistant/controller.ts';
import type { AssistantTurn } from '../src/lib/contracts.ts';

const hostile = 'The plan completed [E1]. The disk will fail tomorrow.';

const turn = (turnId: string, state: number, answer: string): AssistantTurn => ({
  turnId,
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

const kept = (turnId: string): AssistantTurn | undefined => {
  const entry = get(assistantState).transcript.find((e) => e.kind === 'turn' && e.turn.turnId === turnId);
  return entry?.kind === 'turn' ? entry.turn : undefined;
};

test('a streamed frame keeps its progress but not its text', () => {
  applyAssistantTurn(turn('turn-a', TURN_STREAMING, hostile));
  assert.equal(kept('turn-a')?.answer, '');
  assert.equal(kept('turn-a')?.tokensEmitted, 12);
  assert.equal(get(assistantState).inFlight, 'turn-a');
});

test('a turn that ends cancelled or faulted after streaming keeps no text', () => {
  for (const [id, state] of [['turn-b', TURN_CANCELLED], ['turn-c', TURN_FAULTED]] as const) {
    applyAssistantTurn(turn(id, TURN_STREAMING, hostile));
    applyAssistantTurn(turn(id, state, 'deadline exceeded after 40 tokens'));
    assert.equal(kept(id)?.answer, '', id);
    assert.equal(kept(id)?.state, state, id);
  }
});

test('an answered turn keeps the text the gate admitted, in either language', () => {
  for (const [id, answer] of [['turn-d', 'A plan completed [E1].'], ['turn-e', 'اكتملت خطة صيانة [E1].']]) {
    applyAssistantTurn(turn(id, TURN_STREAMING, hostile));
    applyAssistantTurn(turn(id, TURN_ANSWERED, answer));
    assert.equal(kept(id)?.answer, answer, id);
  }
});
