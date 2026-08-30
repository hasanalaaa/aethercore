<script lang="ts">
  import { onDestroy, onMount } from 'svelte';
  import { readMotionPreferences, subscribeMotionPreferences, SpringValue } from '../motion';

  let progress = 0;
  const spring = new SpringValue(0, { response: 0.34, damping: 1, restDelta: 0.001, restSpeed: 0.001 });
  const unsubscribe = spring.subscribe((value) => progress = value);
  let reducedMotion = readMotionPreferences().reducedMotion;
  const unsubscribePreferences = subscribeMotionPreferences((preferences) => {
    reducedMotion = preferences.reducedMotion;
    if (reducedMotion) spring.set(1);
  });
  $: visualProgress = Math.max(0, Math.min(1, progress));
  $: opacity = visualProgress.toFixed(4);
  $: translate = ((1 - visualProgress) * 6).toFixed(3);

  onMount(() => {
    if (reducedMotion) spring.set(1);
    else spring.retarget(1);
  });
  onDestroy(() => { unsubscribe(); unsubscribePreferences(); spring.destroy(); });
</script>

<div class="ac-page-frame" style={`--ac-page-progress:${visualProgress};opacity:${opacity};transform:translate3d(0,${translate}px,0)`}>
  <slot />
</div>
