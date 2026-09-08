<script lang="ts">
  // Phase 34 — Fleet management surface.
  // Local administrative actions (add / edit non-secret metadata / remove
  // with explicit intent / trust & untrust) mutate the SAME strict-domain
  // inventory file the CLI uses, through typed Tauri commands. Trust
  // authorization REQUIRES public key material + fingerprint (the backend
  // recomputes the fingerprint from the key — a fingerprint alone can never
  // create trust). Remote operations (probe/audit/compliance) stay in
  // aetherctl and remain strictly READ-ONLY. No secret field is ever
  // rendered, transferred, or stored by this page.
  import { onMount } from 'svelte';
  import { uiTransport } from '../../platform/transport';
  import { shellState } from '../../app/shell-state';
  import { TechnicalText, Pressable } from '../../design/primitives';
  import { fluidPress } from '../../design/motion';
  import { hasMessageKey, t, td } from '../../lib/i18n';
  import { EmptyState } from '../../design/signature';

  type FleetHostRow = {
    hostId: string;
    displayName: string;
    hostname: string;
    port: number;
    username: string;
    enabled: boolean;
    trusted: boolean;
    fingerprint: string | null;
    keyType: string | null;
    tags: string[];
  };
  type FleetSnapshot = {
    hosts: FleetHostRow[];
    schedules: FleetScheduleRow[];
    scheduleCount: number;
    // Present only when the schedules could not be read: an empty list plus
    // this set is "we could not tell", not "you have none" (DBT-P46-B30).
    schedulesError: string | null;
    sshAvailable: boolean;
  };
  type FleetScheduleRow = {
    scheduleId: string;
    scope: string[];
    profileId: string;
    enabled: boolean;
    cadence: string;
    nextRunUnixMs: number;
    lastResult: { outcomeSummary: string; hostsAttempted: number; hostsOk: number; hostsFailed: number } | null;
  };
  type ActionResult = { ok: boolean; hostId: string; detail: string };
  type RemoteResult = {
    hostId: string;
    operation: string;
    outcome: string;
    detail: string | null;
    stdout: string | null;
    stderr: string | null;
  };
  type ScheduleActionResult = { ok: boolean; scheduleId: string; detail: string };

  type StatusTone = 'ok' | 'warn' | 'error' | 'muted';

  let snapshot: FleetSnapshot | null = null;
  let loadError: string | null = null;
  let notice: { ok: boolean; text: string } | null = null;
  let busy = false;
  let remoteBusy = '';
  let remoteResults: Record<string, RemoteResult> = {};
  let complianceProfile = 'cis-l1';
  let editingHostId = '';
  let editName = '';
  let editTags = '';
  let scheduleFormOpen = false;
  let scheduleEditId = '';
  let scheduleId = '';
  let scheduleScope = '';
  let scheduleProfile = 'cis-l1';
  let scheduleEveryHours = '24';
  let scheduleEnabled = true;

  // add / edit form state (no secret field exists anywhere on this form)
  let showAddForm = false;
  let addId = '';
  let addName = '';
  let addHostname = '';
  let addPort = '22';
  let addUser = '';
  let addAuthKind: 'agent' | 'key' | 'cert' = 'agent';
  let addAuthPath = '';
  let addTags = '';

  // trust flow state: public key + fingerprint must BOTH be provided
  let trustHostId = '';
  let trustKey = '';
  let trustFingerprint = '';

  $: locale = $shellState.locale;

  async function refresh(): Promise<void> {
    try {
      snapshot = await uiTransport.invoke<FleetSnapshot>('fleet_snapshot');
      loadError = null;
    } catch (error) {
      loadError = String(error);
    }
  }

  onMount(refresh);

  function notify(ok: boolean, text: string): void {
    notice = { ok, text };
    setTimeout(() => {
      if (notice && notice.text === text) notice = null;
    }, 6000);
  }

  async function runAction(command: string, args: Record<string, unknown>, okText: string): Promise<ActionResult | null> {
    busy = true;
    try {
      const result = await uiTransport.invoke<ActionResult>(command, args);
      notify(result.ok, result.ok ? okText : result.detail);
      if (result.ok) await refresh();
      return result;
    } catch (error) {
      notify(false, String(error));
      return null;
    } finally {
      busy = false;
    }
  }

  function resetAddForm(): void {
    showAddForm = false;
    addId = '';
    addName = '';
    addHostname = '';
    addPort = '22';
    addUser = '';
    addAuthKind = 'agent';
    addAuthPath = '';
    addTags = '';
  }

  async function submitAdd(): Promise<void> {
    if (!addId.trim() || !addHostname.trim() || !addUser.trim()) {
      notify(false, t('fleet.errRequired', locale));
      return;
    }
    const result = await runAction(
      'fleet_add_host',
      {
        input: {
          hostId: addId.trim(),
          displayName: addName.trim() || addId.trim(),
          hostname: addHostname.trim(),
          port: Number(addPort) || 22,
          username: addUser.trim(),
          authKind: addAuthKind,
          authPath: addAuthKind === 'agent' ? null : addAuthPath.trim() || null,
          tags: addTags.split(',').map((tag) => tag.trim()).filter(Boolean),
        },
      },
      t('fleet.okAdded', locale),
    );
    if (result?.ok) resetAddForm();
  }

  async function toggleEnabled(host: FleetHostRow): Promise<void> {
    await runAction(
      'fleet_edit_host',
      { input: { hostId: host.hostId, enabled: !host.enabled } },
      t('fleet.okEdited', locale),
    );
  }

  async function removeHost(host: FleetHostRow): Promise<void> {
    // explicit intent: two-step confirmation typed by the user
    const confirmed = window.confirm(t('fleet.confirmRemove', locale, { id: host.hostId }));
    if (!confirmed) return;
    await runAction('fleet_remove_host', { hostId: host.hostId, confirm: true }, t('fleet.okRemoved', locale));
  }

  function openTrust(host: FleetHostRow): void {
    trustHostId = trustHostId === host.hostId ? '' : host.hostId;
    trustKey = '';
    trustFingerprint = '';
  }

  async function submitTrust(host: FleetHostRow): Promise<void> {
    if (!trustKey.trim() || !trustFingerprint.trim()) {
      notify(false, t('fleet.errTrustNeedsKey', locale));
      return;
    }
    const result = await runAction(
      'fleet_trust_host',
      {
        input: {
          hostId: host.hostId,
          keyType: host.keyType ?? 'ssh-ed25519',
          publicKeyBase64: trustKey.trim(),
          authorizedFingerprint: trustFingerprint.trim(),
        },
      },
      t('fleet.okTrusted', locale),
    );
    if (result?.ok) {
      trustHostId = '';
      trustKey = '';
      trustFingerprint = '';
    }
  }

  async function untrustHost(host: FleetHostRow): Promise<void> {
    if (!window.confirm(t('fleet.confirmUntrust', locale, { id: host.hostId }))) return;
    await runAction('fleet_untrust_host', { hostId: host.hostId }, t('fleet.okUntrusted', locale));
  }

  function openEdit(host: FleetHostRow): void {
    editingHostId = editingHostId === host.hostId ? '' : host.hostId;
    editName = host.displayName;
    editTags = host.tags.join(', ');
  }

  async function saveEdit(host: FleetHostRow): Promise<void> {
    const result = await runAction('fleet_edit_host', { input: { hostId: host.hostId, displayName: editName.trim(), tags: editTags.split(',').map((tag) => tag.trim()).filter(Boolean) } }, t('fleet.okEdited', locale));
    if (result?.ok) editingHostId = '';
  }

  async function runRemote(host: FleetHostRow, operation: 'probe' | 'audit' | 'compliance'): Promise<void> {
    remoteBusy = `${host.hostId}:${operation}`;
    try {
      const command = operation === 'probe' ? 'fleet_probe' : operation === 'audit' ? 'fleet_audit' : 'fleet_compliance';
      const args = operation === 'compliance' ? { hostId: host.hostId, profileId: complianceProfile } : { hostId: host.hostId };
      const result = await uiTransport.invoke<RemoteResult>(command, args);
      remoteResults = { ...remoteResults, [host.hostId]: result };
    } catch (error) {
      remoteResults = { ...remoteResults, [host.hostId]: { hostId: host.hostId, operation, outcome: 'failed', detail: String(error), stdout: null, stderr: null } };
    } finally {
      remoteBusy = '';
    }
  }

  function resultLabel(outcome: string): string {
    const key = `fleet.result${outcome.split('_').map((part) => part.charAt(0).toUpperCase() + part.slice(1)).join('')}`;
    return hasMessageKey(key) ? td(key, locale) : outcome;
  }

  function resultTone(outcome: string): StatusTone {
    if (outcome === 'success') return 'ok';
    if (outcome === 'failed' || outcome === 'auth_failure' || outcome === 'host_key_mismatch') return 'error';
    return 'warn';
  }

  function resetScheduleForm(): void {
    scheduleFormOpen = false; scheduleEditId = ''; scheduleId = ''; scheduleScope = ''; scheduleProfile = 'cis-l1'; scheduleEveryHours = '24'; scheduleEnabled = true;
  }

  function openScheduleEdit(schedule: FleetScheduleRow): void {
    scheduleFormOpen = true; scheduleEditId = schedule.scheduleId; scheduleId = schedule.scheduleId; scheduleScope = schedule.scope.join(','); scheduleProfile = schedule.profileId; scheduleEveryHours = schedule.cadence.startsWith('every_hours:') ? schedule.cadence.split(':')[1] : '24'; scheduleEnabled = schedule.enabled;
  }

  async function saveSchedule(): Promise<void> {
    const id = scheduleId.trim();
    if (!id) { notify(false, t('fleet.errScheduleRequired', locale)); return; }
    const command = scheduleEditId ? 'fleet_schedule_update' : 'fleet_schedule_add';
    const result = await uiTransport.invoke<ScheduleActionResult>(command, { input: { scheduleId: id, scope: scheduleScope.split(',').map((item) => item.trim()).filter(Boolean), profileId: scheduleProfile, everyHours: Number(scheduleEveryHours) || 24, enabled: scheduleEnabled } });
    notify(result.ok, result.ok ? t(scheduleEditId ? 'fleet.okScheduleUpdated' : 'fleet.okScheduleAdded', locale) : result.detail);
    if (result.ok) { resetScheduleForm(); await refresh(); }
  }

  async function removeSchedule(schedule: FleetScheduleRow): Promise<void> {
    if (!window.confirm(t('fleet.confirmScheduleRemove', locale, { id: schedule.scheduleId }))) return;
    const result = await uiTransport.invoke<ScheduleActionResult>('fleet_schedule_remove', { scheduleId: schedule.scheduleId, confirm: true });
    notify(result.ok, result.ok ? t('fleet.okScheduleRemoved', locale) : result.detail);
    if (result.ok) await refresh();
  }

  async function runDueSchedules(): Promise<void> {
    const result = await uiTransport.invoke<{ ran: number; runs: unknown[] }>('fleet_schedule_run_due');
    notify(true, `${t('fleet.okScheduleRunDue', locale)} (${result.ran})`);
    await refresh();
  }

  function statusTone(host: FleetHostRow): StatusTone {
    if (host.trusted) return 'ok';
    return 'muted';
  }

  function statusLabel(host: FleetHostRow): string {
    if (host.trusted) return t('fleet.stateTrusted', locale);
    return t('fleet.stateUntrusted', locale);
  }
</script>

<header>
  <div>
    <p class="eyebrow">{t('fleet.eyebrow', locale)}</p>
    <h1>{t('fleet.title', locale)}</h1>
    <p class="sub">{t('fleet.subtitle', locale)}</p>
  </div>
  <Pressable className="fleet-add-button" onclick={() => (showAddForm = !showAddForm)} disabled={busy}>
    {showAddForm ? t('fleet.actionCancel', locale) : t('fleet.actionAdd', locale)}
  </Pressable>
</header>

{#if notice}
  <section class="fleet-notice" class:ok={notice.ok} role="status">
    {notice.text}
  </section>
{/if}

{#if loadError}
  <section class="panel fleet-error" role="alert">
    <strong>{t('fleet.loadFailed', locale)}</strong>
    <TechnicalText value={loadError} />
  </section>
{:else if snapshot}
  <section class="hardware-summary fleet-summary">
    <article><span>{t('fleet.hosts', locale)}</span><strong>{snapshot.hosts.length}</strong><small>{t('fleet.hostsHint', locale)}</small></article>
    <article><span>{t('fleet.trusted', locale)}</span><strong>{snapshot.hosts.filter((host) => host.trusted).length}</strong><small>{t('fleet.trustedHint', locale)}</small></article>
    <article><span>{t('fleet.schedules', locale)}</span><strong>{snapshot.scheduleCount}</strong><small>{t('fleet.schedulesHint', locale)}</small></article>
    <article>
      <span>{t('fleet.ssh', locale)}</span>
      <strong>{snapshot.sshAvailable ? t('fleet.sshReady', locale) : t('fleet.sshMissing', locale)}</strong>
      <small>{t('fleet.sshHint', locale)}</small>
    </article>
  </section>

  {#if showAddForm}
    <section class="panel fleet-form" aria-label={t('fleet.actionAdd', locale)}>
      <div class="form-grid">
        <label><span>{t('fleet.fieldId', locale)}</span><input bind:value={addId} placeholder="host-build-01" /></label>
        <label><span>{t('fleet.fieldName', locale)}</span><input bind:value={addName} /></label>
        <label><span>{t('fleet.fieldHost', locale)}</span><input bind:value={addHostname} placeholder="host.example.internal" /></label>
        <label><span>{t('fleet.fieldPort', locale)}</span><input bind:value={addPort} inputmode="numeric" /></label>
        <label><span>{t('fleet.fieldUser', locale)}</span><input bind:value={addUser} /></label>
        <label>
          <span>{t('fleet.fieldAuth', locale)}</span>
          <select bind:value={addAuthKind}>
            <option value="agent">{t('fleet.authAgent', locale)}</option>
            <option value="key">{t('fleet.authKey', locale)}</option>
            <option value="cert">{t('fleet.authCert', locale)}</option>
          </select>
        </label>
        {#if addAuthKind !== 'agent'}
          <label class="form-wide"><span>{t('fleet.fieldAuthPath', locale)}</span><input bind:value={addAuthPath} placeholder="/home/ops/.ssh/id_ed25519" /></label>
        {/if}
        <label class="form-wide"><span>{t('fleet.fieldTags', locale)}</span><input bind:value={addTags} placeholder="edge, build" /></label>
      </div>
      <div class="form-actions">
        <button use:fluidPress={{ pressedScale: 0.985 }} class="primary" onclick={submitAdd} disabled={busy}>{t('fleet.actionSave', locale)}</button>
        <button use:fluidPress={{ pressedScale: 0.985 }} onclick={resetAddForm} disabled={busy}>{t('fleet.actionCancel', locale)}</button>
      </div>
      <p class="form-note">{t('fleet.authPathNote', locale)}</p>
    </section>
  {/if}

  <section class="fleet-list">
    <div class="panel-head">
      <div>
        <p class="eyebrow">{t('fleet.inventoryEyebrow', locale)}</p>
        <h3>{t('fleet.inventoryTitle', locale)}</h3>
      </div>
    </div>
    {#if snapshot.hosts.length === 0}
      <EmptyState title={t('fleet.emptyTitle', locale)} body={t('fleet.emptyCopy', locale)} />
    {:else}
      {#each snapshot.hosts as host (host.hostId)}
        {@const remote = remoteResults[host.hostId]}
        <article class="fleet-card">
          <div class="fleet-card-main">
            <strong>{host.displayName}</strong>
            <small>
              <TechnicalText value={`${host.username}@${host.hostname}:${host.port}`} />
              {#if host.tags.length}
                · {host.tags.join(', ')}
              {/if}
            </small>
            <small class="fleet-fingerprint" title={host.fingerprint ?? ''}>
              {host.keyType ? `${host.keyType} · ` : ''}{host.fingerprint ? `${host.fingerprint.slice(0, 16)}…` : ''}
            </small>
          </div>
          <div class="fleet-card-state">
            <em class:ok={statusTone(host) === 'ok'} class:muted={statusTone(host) === 'muted'}>
              {statusLabel(host)}
            </em>
            <em class:ok={host.enabled} class:warn={!host.enabled}>
              {host.enabled ? t('fleet.stateEnabled', locale) : t('fleet.stateDisabled', locale)}
            </em>
          </div>
          <div class="fleet-card-actions">
            {#if host.trusted}
              <button use:fluidPress={{ pressedScale: 0.985 }} onclick={() => untrustHost(host)} disabled={busy}>{t('fleet.actionUntrust', locale)}</button>
            {:else}
              <button use:fluidPress={{ pressedScale: 0.985 }} class="primary" onclick={() => openTrust(host)} disabled={busy}>{t('fleet.actionTrust', locale)}</button>
            {/if}
            <button use:fluidPress={{ pressedScale: 0.985 }} onclick={() => toggleEnabled(host)} disabled={busy}>
              {host.enabled ? t('fleet.actionDisable', locale) : t('fleet.actionEnable', locale)}
            </button>
            <button use:fluidPress={{ pressedScale: 0.985 }} onclick={() => openEdit(host)} disabled={busy}>{t('fleet.actionEdit', locale)}</button>
            <button use:fluidPress={{ pressedScale: 0.985 }} class="danger" onclick={() => removeHost(host)} disabled={busy}>{t('fleet.actionRemove', locale)}</button>
            <button use:fluidPress={{ pressedScale: 0.985 }} onclick={() => runRemote(host, 'probe')} disabled={!!remoteBusy}>{t('fleet.actionProbe', locale)}</button>
            <button use:fluidPress={{ pressedScale: 0.985 }} onclick={() => runRemote(host, 'audit')} disabled={!!remoteBusy}>{t('fleet.actionAudit', locale)}</button>
            <select aria-label={t('fleet.fieldProfile', locale)} bind:value={complianceProfile}>
              <option value="cis-l1">{t('fleet.profileCisL1', locale)}</option>
              <option value="cis-l2">{t('fleet.profileCisL2', locale)}</option>
            </select>
            <button use:fluidPress={{ pressedScale: 0.985 }} onclick={() => runRemote(host, 'compliance')} disabled={!!remoteBusy}>{t('fleet.actionCompliance', locale)}</button>
          </div>
          {#if editingHostId === host.hostId}
            <div class="trust-form">
              <label><span>{t('fleet.fieldName', locale)}</span><input bind:value={editName} /></label>
              <label><span>{t('fleet.fieldTags', locale)}</span><input bind:value={editTags} /></label>
              <div class="form-actions"><button class="primary" onclick={() => saveEdit(host)} disabled={busy}>{t('fleet.actionSave', locale)}</button><button onclick={() => (editingHostId = '')} disabled={busy}>{t('fleet.actionCancel', locale)}</button></div>
            </div>
          {/if}
          {#if trustHostId === host.hostId}
            <div class="trust-form">
              <label><span>{t('fleet.fieldPublicKey', locale)}</span><input bind:value={trustKey} placeholder="AAAAC3NzaC1lZDI1NTE5AAAA…" /></label>
              <label><span>{t('fleet.fieldFingerprint', locale)}</span><input bind:value={trustFingerprint} placeholder="64-char SHA-256 hex" /></label>
              <p class="form-note">{t('fleet.trustNote', locale)}</p>
              <div class="form-actions">
                <button use:fluidPress={{ pressedScale: 0.985 }} class="primary" onclick={() => submitTrust(host)} disabled={busy}>{t('fleet.actionConfirmTrust', locale)}</button>
                <button use:fluidPress={{ pressedScale: 0.985 }} onclick={() => (trustHostId = '')} disabled={busy}>{t('fleet.actionCancel', locale)}</button>
              </div>
            </div>
          {/if}
          {#if remote}
            <div class="remote-result" class:ok={resultTone(remote.outcome) === 'ok'} class:warn={resultTone(remote.outcome) === 'warn'} class:error={resultTone(remote.outcome) === 'error'} role="status">
              <strong>{remote.operation}: {resultLabel(remote.outcome)}</strong>
              {#if remote.detail}<span>{remote.detail}</span>{/if}
              {#if remote.stdout}<pre dir="ltr">{remote.stdout}</pre>{/if}
              {#if remote.stderr}<pre dir="ltr">{remote.stderr}</pre>{/if}
            </div>
          {/if}
        </article>
      {/each}
    {/if}
  </section>

  <section class="panel fleet-hint">
    <p>{t('fleet.cliHint', locale)}</p>
  </section>

  <section class="panel fleet-schedules">
    <div class="panel-head"><div><p class="eyebrow">{t('fleet.scheduleEyebrow', locale)}</p><h3>{t('fleet.scheduleTitle', locale)}</h3></div><div class="form-actions"><button class="primary" onclick={() => (scheduleFormOpen = !scheduleFormOpen)} disabled={busy}>{scheduleFormOpen ? t('fleet.actionCancel', locale) : t('fleet.actionScheduleAdd', locale)}</button><button onclick={runDueSchedules} disabled={busy}>{t('fleet.actionRunDue', locale)}</button></div></div>
    <p class="form-note">{t('fleet.scheduleHint', locale)}</p>
    {#if scheduleFormOpen}
      <div class="form-grid schedule-form">
        <label><span>{t('fleet.fieldScheduleId', locale)}</span><input bind:value={scheduleId} disabled={!!scheduleEditId} /></label>
        <label><span>{t('fleet.fieldScope', locale)}</span><input bind:value={scheduleScope} placeholder="host-a,host-b" /></label>
        <label><span>{t('fleet.fieldProfile', locale)}</span><select bind:value={scheduleProfile}><option value="cis-l1">{t('fleet.profileCisL1', locale)}</option><option value="cis-l2">{t('fleet.profileCisL2', locale)}</option></select></label>
        <label><span>{t('fleet.fieldCadence', locale)}</span><input bind:value={scheduleEveryHours} inputmode="numeric" /></label>
        <label class="schedule-enabled"><span>{t('fleet.fieldEnabled', locale)}</span><input type="checkbox" bind:checked={scheduleEnabled} /></label>
        <div class="form-actions"><button class="primary" onclick={saveSchedule} disabled={busy}>{scheduleEditId ? t('fleet.actionScheduleUpdate', locale) : t('fleet.actionScheduleAdd', locale)}</button><button onclick={resetScheduleForm} disabled={busy}>{t('fleet.actionCancel', locale)}</button></div>
      </div>
    {/if}
    {#if snapshot.schedulesError}
      <p class="form-note schedule-unreadable">{t('fleet.scheduleUnreadable', locale)}<br /><TechnicalText value={snapshot.schedulesError} /></p>
    {:else if snapshot.schedules.length === 0}
      <p class="form-note">{t('fleet.scheduleEmpty', locale)}</p>
    {:else}
      <div class="schedule-list">
        {#each snapshot.schedules as schedule (schedule.scheduleId)}
          <article class="schedule-row">
            <div><strong><TechnicalText value={schedule.scheduleId} /></strong><small>{schedule.scope.join(', ') || t('fleet.scopeAll', locale)} · {schedule.profileId} · {schedule.cadence}</small></div>
            <div class="fleet-card-state"><em class:ok={schedule.enabled} class:warn={!schedule.enabled}>{schedule.enabled ? t('fleet.stateEnabled', locale) : t('fleet.stateDisabled', locale)}</em>{#if schedule.lastResult}<small>{schedule.lastResult.outcomeSummary}</small>{/if}</div>
            <div class="fleet-card-actions"><button onclick={() => openScheduleEdit(schedule)}>{t('fleet.actionEdit', locale)}</button><button class="danger" onclick={() => removeSchedule(schedule)}>{t('fleet.actionScheduleRemove', locale)}</button></div>
          </article>
        {/each}
      </div>
    {/if}
  </section>
{:else}
  <section class="panel"><p>{t('fleet.loading', locale)}</p></section>
{/if}

<style>
  header { display: flex; justify-content: space-between; align-items: flex-start; gap: 1rem; }

  .fleet-error { display: grid; gap: 0.35rem; }
  .fleet-notice {
    border: 1px solid var(--ac-border-default); border-radius: 10px; padding: 0.55rem 0.8rem;
    font-size: 0.85rem; color: var(--ac-text-1); background: var(--ac-material-base);
  }
  .fleet-notice.ok { border-inline-start: 3px solid var(--ac-accent); }
  .fleet-notice:not(.ok) { border-inline-start: 3px solid #c2791f; }
  .fleet-summary { display: grid; grid-template-columns: repeat(auto-fit, minmax(11rem, 1fr)); gap: 0.6rem; }
  .fleet-list { display: grid; gap: 0.5rem; }
  .fleet-card {
    display: grid; grid-template-columns: 1fr auto; gap: 0.35rem 0.75rem;
    padding: 0.65rem 0.85rem;
    border: 1px solid var(--ac-border-default); border-radius: 12px; background: var(--ac-material-base);
  }
  .fleet-card-main { display: grid; gap: 0.15rem; min-width: 0; }
  .fleet-card-main small { color: var(--ac-text-3); }
  .fleet-fingerprint { font-size: 0.72rem; opacity: 0.85; }
  .fleet-card-state { display: grid; gap: 0.15rem; justify-items: end; }
  .fleet-card-state em { font-style: normal; font-size: 0.78rem; color: var(--ac-text-3); }
  .fleet-card-state em.ok { color: var(--ac-accent); }
  .fleet-card-state em.warn { color: #c2791f; }
  .fleet-card-state em.muted { color: var(--ac-text-3); }
  .fleet-card-actions { grid-column: 1 / -1; display: flex; flex-wrap: wrap; gap: 0.4rem; }
  .fleet-card-actions button {
    border: 1px solid var(--ac-border-default); background: transparent; color: var(--ac-text-1);
    border-radius: 8px; padding: 0.25rem 0.6rem; font-size: 0.78rem; cursor: pointer;
  }
  .fleet-card-actions button.primary { border-color: var(--ac-accent); color: var(--ac-accent); }
  .fleet-card-actions button.danger { border-color: #c2791f; color: #c2791f; }
  .fleet-card-actions button:disabled { opacity: 0.5; cursor: not-allowed; }
  .trust-form, .fleet-form {
    grid-column: 1 / -1; display: grid; gap: 0.5rem;
    border-block-start: 1px dashed var(--ac-border-default); padding-top: 0.6rem; margin-top: 0.2rem;
  }
  .fleet-form { border-block-start: none; padding-top: 0; }
  .form-grid { display: grid; grid-template-columns: repeat(auto-fit, minmax(13rem, 1fr)); gap: 0.5rem; }
  .form-grid label, .trust-form label { display: grid; gap: 0.2rem; font-size: 0.78rem; color: var(--ac-text-3); }
  .form-wide { grid-column: 1 / -1; }
  .form-grid input, .form-grid select, .trust-form input {
    border: 1px solid var(--ac-border-default); border-radius: 8px; padding: 0.35rem 0.55rem;
    background: transparent; color: var(--ac-text-1); font-size: 0.85rem;
  }
  .form-actions { display: flex; gap: 0.4rem; }
  .form-actions button {
    border: 1px solid var(--ac-border-default); background: transparent; color: var(--ac-text-1);
    border-radius: 8px; padding: 0.3rem 0.7rem; font-size: 0.8rem; cursor: pointer;
  }
  .form-actions button.primary { border-color: var(--ac-accent); color: var(--ac-accent); }
  .form-actions button:disabled { opacity: 0.5; cursor: not-allowed; }
  .form-note { font-size: 0.72rem; color: var(--ac-text-3); }
  /* A fault, not a policy refusal: it carries the attention colour the
     rest of this page already uses for a real problem. */
  .form-note.schedule-unreadable { color: #c2791f; }
  .fleet-hint { color: var(--ac-text-3); font-size: 0.82rem; }
  .remote-result { grid-column: 1 / -1; display: grid; gap: 0.25rem; border-inline-start: 3px solid #c2791f; padding: 0.45rem 0.65rem; background: color-mix(in srgb, var(--ac-material-base) 92%, #c2791f); font-size: 0.78rem; }
  .remote-result.ok { border-inline-start-color: var(--ac-accent); }
  .remote-result.error { border-inline-start-color: #b74646; }
  .remote-result pre { margin: 0; max-height: 9rem; overflow: auto; white-space: pre-wrap; font-size: 0.7rem; }
  .fleet-schedules { display: grid; gap: 0.55rem; }
  .schedule-form { border-block-start: 1px dashed var(--ac-border-default); padding-top: 0.6rem; }
  .schedule-enabled { display: flex; align-items: center; gap: 0.45rem; }
  .schedule-list { display: grid; gap: 0.45rem; }
  .schedule-row { display: grid; grid-template-columns: 1fr auto; gap: 0.35rem 0.75rem; padding: 0.55rem 0.7rem; border: 1px solid var(--ac-border-default); border-radius: 10px; }
  .schedule-row > div:first-child { display: grid; gap: 0.15rem; }
  .schedule-row small { color: var(--ac-text-3); }
  .schedule-row .fleet-card-actions { grid-column: 1 / -1; }
</style>
