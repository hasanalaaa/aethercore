<script lang="ts">
  /**
   * The assistant drawer — P57, ITEM 1.
   *
   * The engine has generated since P56 (`DBT-P56-002`, narrowed) and there was
   * nothing to type into. This is the surface, built to `DIRECTION.md`
   * §"Where it lives": an overlay on the inline-end edge, 26rem, on every
   * screen, `Ctrl+/` to open, `Escape` to cancel then close.
   *
   * It OVERLAYS. No screen changes layout because the assistant is open — that
   * is why it is fixed to the viewport and not a column in the shell grid, and
   * it is the reason a docked column was rejected: at 1280 the content area is
   * already at its comfortable floor.
   *
   * THE INVARIANT THIS EXISTS UNDER, and the three places it is enforced here:
   *
   * 1. **Streamed text is not an answer.** The citation gate can only run on a
   *    complete answer, so a STREAMING frame renders under its own provisional
   *    treatment — no citation row, no answer label, muted ink and a live
   *    cursor. The renderer keys off `state`, never off whether text is
   *    present.
   * 2. **A turn that cannot be grounded renders as the honest empty state**,
   *    inside the transcript, naming the scan that would produce the evidence.
   *    Not error styling, and NOT violet: an ungroundable question is not a
   *    policy refusal — the product is not protecting the user from it, it
   *    simply has not measured the thing. Violet stays reserved for a refusal
   *    the product made on purpose.
   * 3. **A model failure is a declared fault with a reason key**, never an
   *    empty answer and never a silent degrade to the rule engine.
   *
   * The empty state counts evidence, not capabilities: the surfaces that
   * currently have rows and how many. A capability tour would promise answers
   * the product has no evidence for, which is the one thing it must not do.
   */
  import { onMount, tick } from 'svelte';
  import { fluidPress } from '../../design/motion';
  import { setPage } from '../../app/shell-state';
  import { TechnicalText } from '../../design/primitives';
  import { EmptyState, EvidenceChip } from '../../design/signature';
  import { t, td, tp, type Locale } from '../../lib/i18n';
  import type { AssistantEvidenceRef, AssistantTurn } from '../../lib/contracts';
  import {
    MAX_QUESTION_CHARS,
    REFUSAL_BUSY,
    REFUSAL_MUTATION_ACTIVE,
    REFUSAL_NOT_COVERED,
    TURN_ANSWERED,
    TURN_CANCELLED,
    TURN_FAULTED,
    TURN_REFUSED,
    TURN_STREAMING,
    askAssistant,
    assistantState,
    cancelAssistantTurn,
    loadAssistantPack,
    packCounts,
    segmentAnswer,
  } from './controller';

  export let open = false;
  export let locale: Locale = 'en';
  export let onClose: () => void;

  let draft = '';
  let input: HTMLTextAreaElement | undefined;
  let transcriptEl: HTMLDivElement | undefined;
  let wasOpen = false;
  /** Where focus was when the drawer opened; it goes back there on close (P75). */
  let returnFocus: HTMLElement | null = null;

  $: state = $assistantState;
  $: inFlight = state.inFlight !== '';
  $: counts = packCounts(state.pack);

  /** `Ctrl+/` opens AND focuses the input — the shortcut is the whole entry. */
  $: if (open && !wasOpen) {
    wasOpen = true;
    returnFocus = document.activeElement instanceof HTMLElement ? document.activeElement : null;
    void loadAssistantPack();
    tick().then(() => input?.focus());
  }
  $: if (!open && wasOpen) {
    wasOpen = false;
    const target = returnFocus;
    returnFocus = null;
    if (target?.isConnected) tick().then(() => target.focus());
  }

  /** The newest turn is the one being read. Follows the stream while it grows. */
  $: if (state.transcript.length && transcriptEl) void scrollToLatest();
  async function scrollToLatest(): Promise<void> {
    await tick();
    // Read into a local first. Assigning to a MEMBER of a reactive `let`
    // invalidates that variable in Svelte, so `transcriptEl.scrollTop = …` would
    // re-trigger the statement above that called this — an unbounded loop that
    // hangs the renderer the moment a turn starts streaming. Measured: the page
    // stopped answering CDP entirely on the first question.
    const element = transcriptEl;
    if (element) element.scrollTop = element.scrollHeight;
  }

  onMount(() => { if (open) void loadAssistantPack(); });

  async function send(): Promise<void> {
    const question = draft.trim();
    if (!question || inFlight) return;
    draft = '';
    await askAssistant(question);
  }

  /**
   * Escape cancels a streaming turn, and then closes.
   *
   * Two presses, not one: cancelling leaves a CANCELLED turn in the transcript
   * with the tokens it reached, and closing on the same press would throw that
   * away before it could be read.
   */
  async function keydown(event: KeyboardEvent): Promise<void> {
    if (event.key !== 'Escape') return;
    event.preventDefault();
    event.stopPropagation();
    if (inFlight) { await cancelAssistantTurn(); return; }
    onClose();
  }

  /**
   * Pointer dismissal, as an action rather than an `onclick` attribute — the
   * same shape `FluidDialog` uses, and for the same reason: a presentational
   * surface that carries a handler attribute is an a11y defect, while an action
   * on it is the sanctioned pattern in this shell.
   */
  function dismissOnPointer(node: HTMLElement) {
    const pointerdown = (event: PointerEvent): void => {
      if (event.target !== event.currentTarget) return;
      onClose();
    };
    node.addEventListener('pointerdown', pointerdown);
    return { destroy: () => node.removeEventListener('pointerdown', pointerdown) };
  }

  function composeKeydown(event: KeyboardEvent): void {
    if (event.key !== 'Enter' || event.shiftKey) return;
    event.preventDefault();
    void send();
  }

  /** The chip face: which surface, and which row of it. The raw is the observation. */
  function chipFor(reference: AssistantEvidenceRef): { cite: string; raw: string } {
    const id = reference.evidenceId.length > 12 ? `${reference.evidenceId.slice(0, 12)}…` : reference.evidenceId;
    return { cite: `${reference.surface} · ${id}`, raw: reference.detail };
  }

  function refusalBody(turn: AssistantTurn): string {
    if (turn.refusal === REFUSAL_NOT_COVERED) return t('assistant.refusedNotCovered', locale);
    if (turn.refusal === REFUSAL_MUTATION_ACTIVE) return t('assistant.refusedMutationActive', locale);
    if (turn.refusal === REFUSAL_BUSY) return t('assistant.refusedBusy', locale);
    return t('assistant.refusedNoEvidence', locale);
  }

  /**
   * The fault, in the user's language, from the key the service declared.
   *
   * An unknown key falls back to the generic failure rather than to an empty
   * string: a fault with no reason on screen is the empty answer this feature
   * is forbidden to produce.
   */
  /**
   * What a screen reader hears when a turn settles: the outcome and its text, once. Streaming
   * tokens are not announced — a provisional answer is not an answer (P75: nothing was announced
   * at all, so a non-visual user never learned the answer had arrived).
   */
  function announcement(entry: (typeof state.transcript)[number] | undefined): string {
    if (!entry || entry.kind === 'question') return '';
    const turn = entry.turn;
    if (turn.state === TURN_ANSWERED) {
      const text = segmentAnswer(turn).map((segment) => (segment.kind === 'text' ? segment.text : '')).join('');
      return `${t('assistant.answered', locale)}: ${text}`;
    }
    if (turn.state === TURN_REFUSED) return `${t('common.notCollected', locale)}: ${refusalBody(turn)}`;
    if (turn.state === TURN_FAULTED) return `${t('assistant.faulted', locale)}: ${faultBody(turn)}`;
    if (turn.state === TURN_CANCELLED) return t('assistant.cancelled', locale);
    return '';
  }

  function faultBody(turn: AssistantTurn): string {
    switch (turn.faultKey) {
      case 'assistant.fault.modelUnavailable': return t('assistant.fault.modelUnavailable', locale);
      case 'assistant.fault.deadlineExceeded': return t('assistant.fault.deadlineExceeded', locale);
      case 'assistant.fault.transport': return t('assistant.fault.transport', locale);
      default: return t('assistant.fault.generationFailed', locale);
    }
  }
</script>

{#if open}
  <!-- The sheet's dismiss surface. Present at every width so the pointer has the
       same exit it has from every other overlay in the shell; below the sheet
       breakpoint it also carries the scrim. It is not a modal barrier — the
       screen behind stays usable, which is the entire reason this overlays
       instead of reflowing: the question is about what you are looking at. -->
  <div class="assistant-dismiss" role="presentation" use:dismissOnPointer></div>

  <!-- A div, not an <aside>: `role="dialog"` is an interactive role and a
       landmark element may not take one. The drawer is already announced by its
       label and reached by its own shortcut. -->
  <div
    class="assistant-drawer"
    role="dialog"
    tabindex="-1"
    aria-label={t('assistant.title', locale)}
    onkeydown={keydown}
  >
    <header class="assistant-head">
      <h2 class="assistant-title">{t('assistant.title', locale)}</h2>
      <span class="assistant-engine">
        <!-- What the engine IS. It never becomes `ruleFallback` because a
             generation failed — that is a FAULTED turn with a reason. -->
        {#if state.engineLabel && state.engineLabel !== 'localModel'}
          {t('assistant.engineDisabled', locale)}
        {:else}
          <TechnicalText value={state.engineLabel || '—'} />
        {/if}
      </span>
      <button
        type="button"
        class="assistant-close"
        use:fluidPress={{ pressedScale: 0.94 }}
        aria-label={t('assistant.close', locale)}
        onclick={onClose}
      >
        <svg viewBox="0 0 24 24" width="15" height="15" fill="none" aria-hidden="true"><path d="m7 7 10 10M17 7 7 17"/></svg>
      </button>
    </header>

    <p class="sr-only" role="status" aria-live="polite">{announcement(state.transcript.at(-1))}</p>
    <div class="assistant-transcript" bind:this={transcriptEl} tabindex="-1">
      {#if state.transcript.length === 0}
        {#if state.packRead && counts.length}
          <!-- What it can answer TODAY, in rows it actually has. Not a list of
               capabilities: a capability tour promises answers the product has
               no evidence for. -->
          <div class="assistant-pack">
            {#each counts as entry (entry.surface)}
              <span class="assistant-pack-row">
                <TechnicalText value={entry.surface} />
                <TechnicalText value={String(entry.count)} />
              </span>
            {/each}
          </div>
        {:else}
          <EmptyState
            title={t('common.notCollected', locale)}
            body={t('assistant.refusedNoEvidence', locale)}
            channels={[{ label: td('nav.deepScan', locale) }, { label: td('nav.activity', locale) }]}
          >
            <button use:fluidPress={{ pressedScale: 0.985 }} class="secondary" type="button" onclick={() => setPage('deepScan')}>{t('overview.scanMyPc', locale)}</button>
          </EmptyState>
        {/if}
      {:else}
        {#each state.transcript as entry (entry.id + entry.kind)}
          {#if entry.kind === 'question'}
            <div class="assistant-turn" data-kind="question">
              <span class="assistant-kicker">{t('assistant.asked', locale)}</span>
              <p class="assistant-question">{entry.text}</p>
            </div>
          {:else if entry.turn.state === TURN_STREAMING}
            <!-- PROVISIONAL. No answer label, no citation row: the gate has not
                 run yet, and presenting this as an answer is the one careless
                 move that would put an uncited claim on the screen. -->
            <div class="assistant-turn" data-kind="streaming">
              <span class="assistant-kicker">{t('assistant.generating', locale)}</span>
              <p class="assistant-provisional">{entry.turn.answer}<span class="assistant-cursor" aria-hidden="true">▊</span></p>
            </div>
          {:else if entry.turn.state === TURN_ANSWERED}
            <div class="assistant-turn" data-kind="answered">
              <span class="assistant-kicker" data-tone="healthy">{t('assistant.answered', locale)}</span>
              <!-- No whitespace between the each and its branches: the marker is
                   part of the sentence, and a newline here renders as a space
                   before the full stop that follows it. -->
              <p class="assistant-answer">{#each segmentAnswer(entry.turn) as segment, index (index)}{#if segment.kind === 'text'}{segment.text}{:else}<span class="assistant-marker"><TechnicalText value={`E${segment.index}`} /></span>{/if}{/each}</p>
              <div class="assistant-citations">
                {#each entry.turn.citations as citation (citation.evidenceId + citation.surface)}
                  <EvidenceChip evidence={chipFor(citation)} {locale} />
                {/each}
              </div>
            </div>
          {:else if entry.turn.state === TURN_REFUSED}
            <!-- The honest empty state, inside the transcript. Never violet:
                 nothing was refused on purpose here. -->
            <div class="assistant-turn" data-kind="refused">
              <EmptyState
                title={t('common.notCollected', locale)}
                body={refusalBody(entry.turn)}
                channels={[{ label: td('nav.deepScan', locale) }, { label: td('nav.activity', locale) }]}
              >
                <button use:fluidPress={{ pressedScale: 0.985 }} class="secondary" type="button" onclick={() => setPage('deepScan')}>{t('overview.scanMyPc', locale)}</button>
              </EmptyState>
            </div>
          {:else if entry.turn.state === TURN_FAULTED}
            <div class="assistant-turn" data-kind="faulted">
              <span class="assistant-kicker" data-tone="critical">{t('assistant.faulted', locale)}</span>
              <p class="assistant-fault">{faultBody(entry.turn)}</p>
              <span class="assistant-fault-key"><TechnicalText value={entry.turn.faultKey || 'assistant.fault.generationFailed'} /></span>
            </div>
          {:else if entry.turn.state === TURN_CANCELLED}
            <div class="assistant-turn" data-kind="cancelled">
              <span class="assistant-kicker">{t('assistant.cancelled', locale)}</span>
              <span class="assistant-tokens"><TechnicalText value={tp('unit.token', locale, entry.turn.tokensEmitted)} /></span>
            </div>
          {/if}
        {/each}
      {/if}
    </div>

    <form class="assistant-compose" onsubmit={(event) => { event.preventDefault(); void send(); }}>
      <label class="sr-only" for="assistant-input">{t('assistant.placeholder', locale)}</label>
      <textarea
        id="assistant-input"
        bind:this={input}
        bind:value={draft}
        rows="2"
        maxlength={MAX_QUESTION_CHARS}
        placeholder={t('assistant.placeholder', locale)}
        spellcheck="false"
        onkeydown={composeKeydown}
      ></textarea>
      {#if inFlight}
        <button type="button" class="secondary" use:fluidPress={{ pressedScale: 0.985 }} onclick={() => void cancelAssistantTurn()}>
          {t('assistant.stop', locale)}<kbd>Esc</kbd>
        </button>
      {:else}
        <button type="submit" class="primary" use:fluidPress={{ pressedScale: 0.985 }} disabled={!draft.trim()}>
          {t('assistant.send', locale)}<kbd>↵</kbd>
        </button>
      {/if}
    </form>
  </div>
{/if}

<style>
  /* ---- the overlay ------------------------------------------------------
     Fixed to the viewport, never in the shell grid: no screen may change
     layout because the assistant is open.

     The sheet threshold is 58rem — the same number `DIRECTION.md` names for the
     shell's overlays — read against the VIEWPORT rather than against `main`'s
     container. It has to be: `main` carries `container-type: inline-size`, which
     brings layout containment with it, so a fixed element placed inside `main`
     to reach its container query would be positioned against `main`'s padding
     box and would scroll away with the page instead of staying on the window's
     edge. Same number, the only context this element actually lives in. */
  .assistant-dismiss {
    position: fixed;
    inset: 0;
    z-index: 1100;
    border: 0;
    padding: 0;
    background: transparent;
    cursor: default;
  }

  .assistant-drawer {
    position: fixed;
    inset-block: 0;
    inset-inline-end: 0;
    z-index: 1101;
    inline-size: 26rem;
    max-inline-size: 100vw;
    display: flex;
    flex-direction: column;
    flex-wrap: nowrap;
    border-inline-start: 1px solid var(--ac-edge);
    /* Level 1 of 3 — the drawer's own ground, the same focused material every
       other overlay in the shell paints. */
    background: var(--ac-material-focused);
    backdrop-filter: blur(var(--ac-blur-focused)) saturate(var(--ac-saturation));
    -webkit-backdrop-filter: blur(var(--ac-blur-focused)) saturate(var(--ac-saturation));
    box-shadow: var(--ac-shadow-float);
  }

  @media (max-width: 58rem) {
    /* A full-width sheet, like every other overlay in the shell. The dismiss
       surface carries the scrim here, where the drawer covers the screen it is
       being asked about. */
    .assistant-drawer { inline-size: 100%; border-inline-start: 0; }
    .assistant-dismiss { background: var(--ac-scrim); }
  }

  .assistant-head {
    display: flex;
    align-items: center;
    gap: var(--ac-space-4);
    padding: var(--ac-space-5) var(--ac-space-5) var(--ac-space-4);
    border-block-end: 1px solid var(--ac-edge);
  }
  .assistant-title {
    flex: 1;
    min-inline-size: 0;
    margin: 0;
    font-size: var(--ac-type-headline);
    font-weight: 600;
    color: var(--ac-text-1);
  }
  .assistant-engine { color: var(--ac-text-4); font-size: var(--ac-type-technical); white-space: nowrap; }
  .assistant-engine :global(.technical-isolate) { font-family: var(--ac-font-mono); font-size: var(--ac-type-technical); }

  .assistant-close {
    display: grid;
    place-items: center;
    inline-size: var(--ac-target-min);
    block-size: var(--ac-target-min);
    flex-shrink: 0;
    border: 1px solid var(--ac-edge);
    border-radius: var(--ac-radius-pill);
    /* Level 3 of 3 — the control layer, shared with `.secondary` and the
       evidence chip. */
    background: var(--ac-glass-2);
    color: var(--ac-text-3);
    cursor: pointer;
  }
  .assistant-close svg { stroke: currentColor; stroke-width: 1.7; stroke-linecap: round; }
  .assistant-close:hover { border-color: var(--ac-edge-strong); color: var(--ac-text-2); }

  /* ---- the transcript ---------------------------------------------------
     Space groups: `space-6` between turns, `space-3` inside one. */
  .assistant-transcript {
    flex: 1;
    min-block-size: 0;
    overflow-y: auto;
    display: flex;
    flex-direction: column;
    flex-wrap: nowrap;
    gap: var(--ac-space-6);
    padding: var(--ac-space-6) var(--ac-space-5);
  }

  .assistant-pack { display: flex; flex-direction: column; flex-wrap: nowrap; gap: var(--ac-space-3); }
  .assistant-pack-row {
    display: flex;
    align-items: baseline;
    justify-content: space-between;
    gap: var(--ac-space-4);
    padding: var(--ac-space-3) var(--ac-space-4);
    border: 1px solid var(--ac-edge);
    border-radius: var(--ac-radius-md);
    /* Level 2 of 3 — the well: something recessed inside the drawer. */
    background: var(--ac-sunken);
  }
  .assistant-pack-row :global(.technical-isolate) { font-family: var(--ac-font-mono); font-size: var(--ac-type-technical); color: var(--ac-text-3); }

  .assistant-turn { display: flex; flex-direction: column; flex-wrap: nowrap; gap: var(--ac-space-3); min-inline-size: 0; }

  .assistant-kicker {
    font-family: var(--ac-font-mono);
    font-size: var(--ac-type-kicker);
    letter-spacing: var(--ac-tracking-kicker);
    text-transform: uppercase;
    color: var(--ac-text-4);
  }
  .assistant-kicker[data-tone='healthy'] { color: var(--role-healthy); }
  .assistant-kicker[data-tone='critical'] { color: var(--role-critical); }

  .assistant-question {
    margin: 0;
    padding: var(--ac-space-3) var(--ac-space-4);
    border-radius: var(--ac-radius-md);
    background: var(--ac-sunken);
    font-size: var(--ac-type-body);
    line-height: 1.6;
    color: var(--ac-text-2);
    text-wrap: pretty;
    overflow-wrap: anywhere;
  }

  /* Provisional. Muted ink and a live cursor, and deliberately NOT the answer's
     treatment: nothing here has passed the citation gate yet. */
  .assistant-provisional {
    margin: 0;
    font-size: var(--ac-type-body);
    line-height: 1.7;
    color: var(--ac-text-4);
    text-wrap: pretty;
    overflow-wrap: anywhere;
  }
  .assistant-cursor { margin-inline-start: 0.15em; color: var(--role-interactive); animation: ac-assistant-blink 1.1s steps(1) infinite; }

  .assistant-answer {
    margin: 0;
    font-size: var(--ac-type-body);
    line-height: 1.7;
    color: var(--ac-text-1);
    text-wrap: pretty;
    overflow-wrap: anywhere;
  }
  /* The marker the grounding gate left in the sentence, so a reader can see
     which clause rests on which observation. It is a REFERENCE; the evidence
     itself is the chip below, which expands to the raw reading. */
  .assistant-marker :global(.technical-isolate) {
    font-family: var(--ac-font-mono);
    font-size: var(--ac-type-technical);
    padding-inline: 0.28em;
    border-radius: var(--ac-radius-xs);
    background: var(--role-interactive-wash);
    color: var(--role-interactive);
  }

  .assistant-citations { display: flex; flex-direction: column; align-items: flex-start; gap: var(--ac-space-3); min-inline-size: 0; }

  .assistant-fault {
    margin: 0;
    padding: var(--ac-space-3) var(--ac-space-4);
    border: 1px solid var(--role-critical-edge);
    border-radius: var(--ac-radius-md);
    background: var(--role-critical-wash);
    font-size: var(--ac-type-body);
    line-height: 1.6;
    color: var(--role-critical);
  }
  .assistant-fault-key :global(.technical-isolate),
  .assistant-tokens :global(.technical-isolate) {
    font-family: var(--ac-font-mono);
    font-size: var(--ac-type-technical);
    color: var(--ac-text-4);
    overflow-wrap: anywhere;
  }

  /* ---- compose ---------------------------------------------------------- */
  .assistant-compose {
    display: flex;
    flex-direction: column;
    flex-wrap: nowrap;
    gap: var(--ac-space-3);
    padding: var(--ac-space-4) var(--ac-space-5) var(--ac-space-5);
    border-block-start: 1px solid var(--ac-edge);
  }
  .assistant-compose textarea {
    inline-size: 100%;
    resize: none;
    padding: var(--ac-space-3) var(--ac-space-4);
    border: 1px solid var(--ac-edge);
    border-radius: var(--ac-radius-md);
    background: var(--ac-sunken);
    color: var(--ac-text-1);
    font-family: var(--ac-font-text);
    font-size: var(--ac-type-body);
    line-height: 1.6;
  }
  .assistant-compose textarea:focus-visible { outline: none; border-color: var(--role-interactive-edge); box-shadow: var(--ac-shadow-focus); }
  .assistant-compose button { align-self: flex-end; display: inline-flex; align-items: center; gap: var(--ac-space-3); min-block-size: var(--ac-control-height); padding-inline: var(--ac-space-5); border-radius: var(--ac-radius-md); border: 1px solid transparent; cursor: pointer; font-size: var(--ac-type-body); }
  .assistant-compose kbd { font-family: var(--ac-font-mono); font-size: var(--ac-type-technical); opacity: .7; }

  @keyframes ac-assistant-blink { 0%, 55% { opacity: 1 } 56%, 100% { opacity: 0 } }
  @media (prefers-reduced-motion: reduce) { .assistant-cursor { animation: none; } }
</style>
