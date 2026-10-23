<script lang="ts">
  let {
    name,
    size = 18,
    stroke = 1.8,
  }: { name: string; size?: number; stroke?: number } = $props()
</script>

<svg
  width={size}
  height={size}
  viewBox="0 0 24 24"
  fill="none"
  stroke="currentColor"
  stroke-width={stroke}
  stroke-linecap="round"
  stroke-linejoin="round"
  aria-hidden="true"
>
  {#if name === 'mic'}
    <rect x="9" y="2" width="6" height="12" rx="3" />
    <path d="M5 10v1a7 7 0 0 0 14 0v-1M12 18v4M8 22h8" />
  {:else if name === 'desktop'}
    <rect x="2.5" y="4" width="19" height="13" rx="2" />
    <path d="M8 21h8M12 17v4" />
  {:else if name === 'volume'}
    <path d="M11 5 6 9H2v6h4l5 4V5Z" />
    <path d="M15.5 8.5a5 5 0 0 1 0 7M18.5 5.5a9 9 0 0 1 0 13" />
  {:else if name === 'headphones'}
    <path d="M4 14v-2a8 8 0 0 1 16 0v2" />
    <rect x="3" y="14" width="4" height="7" rx="2" />
    <rect x="17" y="14" width="4" height="7" rx="2" />
  {:else if name === 'record'}
    <circle cx="12" cy="12" r="8" />
    <circle cx="12" cy="12" r="3.5" fill="currentColor" stroke="none" />
  {:else if name === 'stream'}
    <path d="M4.5 8a9 9 0 0 1 15 0M7.5 11.5a5 5 0 0 1 9 0M10.5 15a1.8 1.8 0 0 1 3 0" />
    <circle cx="12" cy="18.5" r="1" fill="currentColor" stroke="none" />
    <path d="M2 6.5a13 13 0 0 1 20 0" opacity=".55" />
  {:else if name === 'projector'}
    <rect x="3" y="6" width="18" height="12" rx="2" />
    <circle cx="12" cy="12" r="3" />
    <path d="M7 22h10M12 18v4" />
  {:else if name === 'settings'}
    <circle cx="12" cy="12" r="3" />
    <path
      d="M19.4 15a1.7 1.7 0 0 0 .34 1.88l.06.06-2.83 2.83-.06-.06a1.7 1.7 0 0 0-1.88-.34 1.7 1.7 0 0 0-1 1.55V21h-4v-.08a1.7 1.7 0 0 0-1.06-1.55 1.7 1.7 0 0 0-1.88.34l-.06.06-2.83-2.83.06-.06A1.7 1.7 0 0 0 4.6 15a1.7 1.7 0 0 0-1.55-1H3v-4h.08A1.7 1.7 0 0 0 4.6 8.94a1.7 1.7 0 0 0-.34-1.88L4.2 7l2.83-2.83.06.06A1.7 1.7 0 0 0 8.97 4.6 1.7 1.7 0 0 0 10 3.05V3h4v.08a1.7 1.7 0 0 0 1.06 1.55 1.7 1.7 0 0 0 1.88-.34l.06-.06L19.8 7l-.06.06a1.7 1.7 0 0 0-.34 1.88A1.7 1.7 0 0 0 20.95 10H21v4h-.08a1.7 1.7 0 0 0-1.52 1Z"
    />
  {:else if name === 'chevron-down'}
    <path d="m6 9 6 6 6-6" />
  {:else if name === 'chevron-right'}
    <path d="m9 6 6 6-6 6" />
  {:else if name === 'check'}
    <path d="m5 12 4.5 4.5L19 7" />
  {:else if name === 'x'}
    <path d="M6 6l12 12M18 6 6 18" />
  {:else if name === 'circle'}
    <circle cx="12" cy="12" r="8" />
  {:else if name === 'square'}
    <rect x="4.5" y="4.5" width="15" height="15" rx="4" />
  {:else if name === 'refresh'}
    <path d="M20 11a8 8 0 1 0-2.34 5.66M20 4v7h-7" />
  {:else if name === 'alert'}
    <path d="M12 3 2.5 20h19L12 3Z" />
    <path d="M12 9v5M12 17.2v.1" />
  {:else if name === 'sparkles'}
    <path d="m12 3 1.5 4.5L18 9l-4.5 1.5L12 15l-1.5-4.5L6 9l4.5-1.5L12 3Z" />
    <path d="M5 16.5 6 19l2.5 1-2.5 1L5 23.5 4 21l-2.5-1L4 19l1-2.5Z" opacity=".65" />
  {:else if name === 'layout'}
    <rect x="3" y="3" width="18" height="18" rx="2" />
    <path d="M3 9h18M9 9v12" />
  {:else if name === 'camera'}
    <path d="M4 7h3l1.5-2h7L17 7h3a1 1 0 0 1 1 1v10a1 1 0 0 1-1 1H4a1 1 0 0 1-1-1V8a1 1 0 0 1 1-1Z" />
    <circle cx="12" cy="13" r="3.5" />
  {:else if name === 'screen'}
    <rect x="3" y="4" width="18" height="13" rx="2" />
    <path d="M8 21h8M12 17v4" />
  {:else if name === 'lock'}
    <rect x="4.5" y="10" width="15" height="11" rx="2" />
    <path d="M8 10V7a4 4 0 0 1 8 0v3" />
  {:else if name === 'monitor'}
    <path d="M4 14v-2a8 8 0 0 1 16 0v2" />
    <rect x="3" y="14" width="4" height="7" rx="2" />
    <rect x="17" y="14" width="4" height="7" rx="2" />
  {:else if name === 'external'}
    <path d="M14 4h6v6M20 4l-9 9" />
    <path d="M19 14v4a2 2 0 0 1-2 2H6a2 2 0 0 1-2-2V7a2 2 0 0 1 2-2h4" />
  {:else if name === 'eye'}
    <path d="M2.5 12S6 5.5 12 5.5 21.5 12 21.5 12 18 18.5 12 18.5 2.5 12 2.5 12Z" />
    <circle cx="12" cy="12" r="3" />
  {:else if name === 'eye-off'}
    <path d="m3 3 18 18M10.5 6.1A9.6 9.6 0 0 1 12 6c6 0 9.5 6 9.5 6a17 17 0 0 1-3.2 3.9M6.5 7.7A17 17 0 0 0 2.5 12S6 18 12 18c1.2 0 2.3-.24 3.3-.64" />
    <path d="M9.9 9.9a3 3 0 0 0 4.2 4.2" />
  {:else if name === 'sliders'}
    <path d="M4 6h6M14 6h6M4 12h11M19 12h1M4 18h2M10 18h10" />
    <circle cx="12" cy="6" r="2" />
    <circle cx="17" cy="12" r="2" />
    <circle cx="8" cy="18" r="2" />
  {:else if name === 'scissors'}
    <circle cx="6" cy="7" r="2.5" />
    <circle cx="6" cy="17" r="2.5" />
    <path d="m8.2 8.5 11 7.5M8.2 15.5l11-7.5" />
  {:else if name === 'folder'}
    <path d="M3 7a2 2 0 0 1 2-2h4l2 2h8a2 2 0 0 1 2 2v8a2 2 0 0 1-2 2H5a2 2 0 0 1-2-2Z" />
  {:else if name === 'image'}
    <rect x="3" y="4" width="18" height="16" rx="2" />
    <circle cx="8.5" cy="9" r="1.5" />
    <path d="m21 15-5-5L5 20" />
  {:else if name === 'power'}
    <path d="M12 2v9" />
    <path d="M6.3 5.7a8 8 0 1 0 11.4 0" />
  {:else}
    <circle cx="12" cy="12" r="8" />
  {/if}
</svg>
