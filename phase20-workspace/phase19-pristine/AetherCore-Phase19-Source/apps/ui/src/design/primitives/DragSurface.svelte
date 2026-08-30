<script lang="ts">
  import { fluidDrag, type FluidDragAxis } from '../motion';

  export let axis: FluidDragAxis = 'y';
  export let initial = 0;
  export let min = Number.NEGATIVE_INFINITY;
  export let max = Number.POSITIVE_INFINITY;
  export let snapPoints: readonly number[] = [];
  export let className = '';
  export let onPosition: (value: number, settled: boolean) => void = () => {};
</script>

<div
  class={`ac-drag-surface ${className}`}
  use:fluidDrag={{ axis, initial, min, max, snapPoints, onPosition }}
>
  <slot />
</div>
