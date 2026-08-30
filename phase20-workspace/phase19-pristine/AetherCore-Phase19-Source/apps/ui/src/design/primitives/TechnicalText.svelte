<script lang="ts">
  export let value = '';
  export let as: 'span' | 'code' = 'span';
  export let title: string | undefined = undefined;
</script>

{#if as === 'code'}
  <code class="technical-isolate" dir="ltr" translate="no" data-technical {title}>{value}</code>
{:else}
  <span class="technical-isolate" dir="ltr" translate="no" data-technical {title}>{value}</span>
{/if}
