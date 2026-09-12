<script context="module" lang="ts">
  /** Three neutral levels and the focused overlay. `elevated` was a fourth
   *  fill half a step above `base`; it bought no distinction and cost every
   *  screen a surface level (DIRECTION.md, principle 3). */
  export type MaterialLevel = 'base' | 'structural' | 'focused';
</script>

<script lang="ts">
  export let level: MaterialLevel = 'base';
  export let className = '';
  export let as: 'section' | 'article' | 'div' = 'section';
</script>

<svelte:element this={as} class={`ac-material ac-material-${level} ${className}`}>
  <slot />
</svelte:element>
