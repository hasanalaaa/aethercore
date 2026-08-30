<script lang="ts">
  export type MaterialLevel = 'base' | 'structural' | 'elevated' | 'focused';
  export let level: MaterialLevel = 'base';
  export let className = '';
  export let as: 'section' | 'article' | 'div' = 'section';
</script>

<svelte:element this={as} class={`ac-material ac-material-${level} ${className}`}>
  <slot />
</svelte:element>
