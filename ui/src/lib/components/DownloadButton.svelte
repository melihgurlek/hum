<script lang="ts">
  import { downloadOf, setDownload } from '../state.svelte'
  import Icon from './Icon.svelte'

  /** `target`: 'liked' or `playlist:<id>`. */
  let { target }: { target: string } = $props()

  const d = $derived(downloadOf(target))
  const whole = $derived(!!d && d.done >= d.total)
  const label = $derived(!d ? 'Download' : whole ? 'Downloaded' : `Downloading ${d.done} of ${d.total}`)
</script>

<!-- Off: Download. On: Downloading n of m, then Downloaded; a tap on either takes it off this
     device again (the songs stay in the playlist). -->
<button class="dl" class:on={!!d} aria-pressed={!!d}
  title={d ? 'Remove the download from this device' : 'Keep these songs on this device to play offline'}
  onclick={() => setDownload(target, !d)}>
  {#if d && !whole}<span class="spin" aria-hidden="true"></span>{:else}<Icon name={whole ? 'check' : 'download'} size={16} />{/if}
  <span class="trim">{label}</span>
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
  .dl:hover {
    color: var(--text);
    background: var(--raised);
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
