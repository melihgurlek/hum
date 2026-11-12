<script lang="ts">
  import { downloadLabel, downloadOf, downloads, setDownload } from '../state.svelte'
  import Icon from './Icon.svelte'

  /** `target`: 'liked' or `playlist:<id>`. `compact`: the icon only (the phone's playlist
      header has no room for the words, so it shows them under the name). */
  let { target, compact = false }: { target: string; compact?: boolean } = $props()

  const d = $derived(downloadOf(target))
  const whole = $derived(!!d && d.done >= d.total)
  const label = $derived(downloadLabel(target))
  const waiting = $derived(!!d && !whole && !!downloads.status?.waiting_for_wifi)
</script>

<!-- Off: Download. On: Downloading n of m, then Downloaded; a tap on either takes it off this
     device again (the songs stay in the playlist). -->
<button class="dl" class:on={!!d} class:compact aria-pressed={!!d} aria-label={compact ? label : undefined}
  title={d ? 'Remove the download from this device' : 'Keep these songs on this device to play offline'}
  onclick={() => setDownload(target, !d)}>
  {#if d && !whole && !waiting}<span class="spin" aria-hidden="true"></span>{:else if waiting}<Icon name="download" size={16} />{:else}<Icon name={whole ? 'check' : 'download'} size={16} />{/if}
  {#if !compact}<span class="trim">{label}</span>{/if}
</button>

<style>
  .dl {
    display: inline-flex;
    align-items: center;
    gap: 8px;
    height: 44px;
    padding: 0 14px;
    border-radius: 999px;
    font-weight: 700;
    color: var(--muted);
  }
  .dl.compact {
    width: 44px;
    padding: 0;
    justify-content: center;
  }
  /* Only with a real pointer: on a touchscreen :hover sticks after the tap, which left the
     button lit after a download was removed. */
  @media (hover: hover) {
    .dl:hover {
      color: var(--text);
      background: var(--raised);
    }
  }
  .dl.on {
    color: var(--accent, var(--accent-default));
  }
  .spin {
    width: 14px;
    height: 14px;
    border-radius: 50%;
    border: 2px solid currentColor;
    border-right-color: transparent;
    animation: spin 0.8s linear infinite;
  }
  @keyframes spin {
    to {
      transform: rotate(360deg);
    }
  }
</style>
