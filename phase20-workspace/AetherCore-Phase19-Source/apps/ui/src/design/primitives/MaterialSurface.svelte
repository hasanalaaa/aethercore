<script context="module" lang="ts">
  export type MaterialLevel = 'base' | 'structural' | 'elevated' | 'focused';
</script>

<script lang="ts">
  export let level: MaterialLevel = 'base';
  export let className = '';
  export let as: 'section' | 'article' | 'div' = 'section';
</script>

<svelte:element this={as} class={`ac-material ac-material-${level} ${className}`}>
  <slot />
</svelte:element>
