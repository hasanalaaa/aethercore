<script lang="ts">
  import type { IconName } from '../lib/navigation';
  export let name: IconName;
  export let size = 18;
</script>

<svg class="app-icon" width={size} height={size} viewBox="0 0 24 24" fill="none" aria-hidden="true" focusable="false">
  {#if name === 'overview'}
    <path d="M4 5.5A1.5 1.5 0 0 1 5.5 4h4A1.5 1.5 0 0 1 11 5.5v4A1.5 1.5 0 0 1 9.5 11h-4A1.5 1.5 0 0 1 4 9.5v-4Zm9 0A1.5 1.5 0 0 1 14.5 4h4A1.5 1.5 0 0 1 20 5.5v4a1.5 1.5 0 0 1-1.5 1.5h-4A1.5 1.5 0 0 1 13 9.5v-4Zm-9 9A1.5 1.5 0 0 1 5.5 13h4a1.5 1.5 0 0 1 1.5 1.5v4A1.5 1.5 0 0 1 9.5 20h-4A1.5 1.5 0 0 1 4 18.5v-4Zm9 0a1.5 1.5 0 0 1 1.5-1.5h4a1.5 1.5 0 0 1 1.5 1.5v4a1.5 1.5 0 0 1-1.5 1.5h-4a1.5 1.5 0 0 1-1.5-1.5v-4Z"/>
  {:else if name === 'scan'}
    <circle cx="12" cy="12" r="7"/><circle cx="12" cy="12" r="2"/><path d="M12 2v3M12 19v3M2 12h3M19 12h3"/>
  {:else if name === 'drivers'}
    <rect x="4" y="6" width="16" height="12" rx="3"/><path d="M8 10h8M8 14h5M7 3v3M17 3v3M7 18v3M17 18v3"/>
  {:else if name === 'repair'}
    <path d="M14.8 5.2a4.8 4.8 0 0 0-5.7 6.3L4.6 16a2 2 0 1 0 2.8 2.8l4.5-4.5a4.8 4.8 0 0 0 6.3-5.7l-2.9 2.9-2.8-2.8 2.3-3.5Z"/>
  {:else if name === 'cleanup'}
    <path d="M9 4h6l1 3H8l1-3ZM6 7h12l-1 13H7L6 7Z"/><path d="M10 10v7M14 10v7"/>
  {:else if name === 'startup'}
    <path d="M5 19 19 5M11 5h8v8"/><path d="M5 8v11h11"/>
  {:else if name === 'hardware'}
    <rect x="5" y="5" width="14" height="14" rx="3"/><path d="M9 9h6v6H9zM9 2v3M15 2v3M9 19v3M15 19v3M2 9h3M2 15h3M19 9h3M19 15h3"/>
  {:else if name === 'crash'}
    <path d="M4 14h4l2-6 4 10 2-4h4"/><path d="M4 5h16v14H4z"/>
  {:else if name === 'activity'}
    <path d="M12 4a8 8 0 1 1-7.3 4.7"/><path d="M4 4v5h5M12 8v4l3 2"/>
  {:else if name === 'search'}
    <circle cx="11" cy="11" r="6"/><path d="m16 16 4 4"/>
  {:else if name === 'language'}
    <circle cx="12" cy="12" r="9"/><path d="M3 12h18M12 3c2.2 2.4 3.2 5.4 3.2 9S14.2 18.6 12 21c-2.2-2.4-3.2-5.4-3.2-9S9.8 5.4 12 3Z"/>
  {:else if name === 'theme'}
    <path d="M12 3a9 9 0 1 0 9 9 7 7 0 0 1-9-9Z"/><path d="M17.5 4.5v3M16 6h3"/>
  {:else if name === 'check'}
    <path d="m5 12 4 4 10-10"/>
  {:else if name === 'warning'}
    <path d="M12 4 3.8 19h16.4L12 4Z"/><path d="M12 9v4M12 16.5v.1"/>
  {:else if name === 'fleet'}
    <rect x="3" y="7" width="12" height="10" rx="2"/><path d="M15 10h3.6l2.4 3v4h-3"/><circle cx="7.5" cy="17.5" r="1.6"/><circle cx="17" cy="17.5" r="1.6"/>
  {:else}
    <rect x="5" y="6" width="14" height="12" rx="3"/><path d="M8 10h8M8 14h8"/>
  {/if}
</svg>

<style>
  .app-icon{display:block;stroke:currentColor;stroke-width:1.65;stroke-linecap:round;stroke-linejoin:round;fill:none}
</style>
