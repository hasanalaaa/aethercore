<script lang="ts">
  import { fluidPress, type FluidPressOptions } from '../motion/fluid-press';

  export let disabled = false;
  export let type: 'button' | 'submit' | 'reset' = 'button';
  export let className = '';
  export let ariaLabel: string | undefined = undefined;
  export let title: string | undefined = undefined;
  export let pressedScale = 0.975;
  export let onclick: ((event: MouseEvent) => void) | undefined = undefined;

  $: pressOptions = { pressedScale, hysteresis: 10 } satisfies FluidPressOptions;
</script>

<button
  {type}
  {disabled}
  class={`ac-pressable ${className}`}
  aria-label={ariaLabel}
  {title}
  use:fluidPress={pressOptions}
  {onclick}
>
  <slot />
</button>
