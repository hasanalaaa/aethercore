<script lang="ts">
  import { onDestroy } from 'svelte';
  import { readMotionPreferences, subscribeMotionPreferences, SpringValue } from '../motion';

  export let value = 0;
  export let known = true;
  export let label: string;

  let target = Math.max(0, Math.min(100, Number.isFinite(value) ? value : 0));
  let presentation = target;
  let reducedMotion = readMotionPreferences().reducedMotion;
  const spring = new SpringValue(presentation, { response: 0.32, damping: 1, restDelta: 0.02, restSpeed: 0.02 });
  const unsubscribe = spring.subscribe((next) => presentation = next);
  const unsubscribePreferences = subscribeMotionPreferences((preferences) => {
    reducedMotion = preferences.reducedMotion;
    if (reducedMotion) spring.set(target);
  });

  $: target = Math.max(0, Math.min(100, Number.isFinite(value) ? value : 0));
  $: {
    if (reducedMotion) spring.set(target);
    else spring.retarget(target);
  }

  onDestroy(() => { unsubscribe(); unsubscribePreferences(); spring.destroy(); });
</script>

{#if known}
  <div class="ac-progress" role="progressbar" aria-label={label} aria-valuemin="0" aria-valuemax="100" aria-valuenow={Math.round(target)}>
    <span style={`transform:scaleX(${presentation / 100})`}></span>
  </div>
{:else}
  <div class="ac-progress ac-progress-indeterminate" role="progressbar" aria-label={label}><span></span></div>
{/if}
