<script lang="ts">
  // The fill moves once a second, on the engine's position reports. No CSS transition: a
  // transition restarted every second keeps the webview drawing 60 frames a second for the
  // whole song (~19 % of a core on Linux, ~28 % on Windows, vs ~2 % stepping).
  import { api } from '../api'
  import { player } from '../state.svelte'

  let { tone = 'bar' }: { tone?: 'bar' | 'np' } = $props()

  const fraction = $derived(player.duration ? Math.min(1, Math.max(0, player.position / player.duration)) : 0)

  let el: HTMLDivElement
  function seek(e: MouseEvent) {
    if (!player.duration) return
    const r = el.getBoundingClientRect()
    const f = Math.min(1, Math.max(0, (e.clientX - r.left) / r.width))
    api.seek(f * player.duration)
  }
  function key(e: KeyboardEvent) {
    if (e.key === 'ArrowRight') api.seek(player.position + 5)
    if (e.key === 'ArrowLeft') api.seek(Math.max(0, player.position - 5))
  }
</script>

<div
  class="track {tone}"
  bind:this={el}
  onclick={seek}
  onkeydown={key}
  role="slider"
  tabindex="0"
  aria-label="Seek"
  aria-valuemin="0"
  aria-valuemax={Math.round(player.duration)}
  aria-valuenow={Math.round(player.position)}
>
  <div class="fill" style="transform: scaleX({fraction})"></div>
</div>

<style>
  .track {
    flex: 1;
    height: 16px;
    display: flex;
    align-items: center;
    cursor: pointer;
    position: relative;
  }
  .track::before {
    content: '';
    position: absolute;
    left: 0;
    right: 0;
    height: 4px;
    border-radius: 2px;
    background: var(--line);
  }
  .np::before {
    background: color-mix(in srgb, var(--np-text) 22%, transparent);
    height: 6px;
  }
  .fill {
    position: absolute;
    left: 0;
    right: 0;
    height: 4px;
    border-radius: 2px;
    background: var(--text);
    transform-origin: left center;
  }
  .np .fill {
    background: var(--np-text);
    height: 6px;
  }
  .track:hover .fill {
    background: var(--accent, var(--accent-default));
  }
</style>
