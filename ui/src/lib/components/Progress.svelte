<script lang="ts">
  // The fill glides by CSS transition between the engine's once-a-second position reports,
  // so nothing re-renders in between and nothing repaints at all while paused.
  import { api } from '../api'
  import { player } from '../state.svelte'

  let { tone = 'bar' }: { tone?: 'bar' | 'np' } = $props()

  const gliding = $derived(!player.paused && !player.loading && player.duration > 0)
  const fraction = $derived.by(() => {
    if (!player.duration) return 0
    // Aim one second ahead and let the 1 s linear transition carry the fill there.
    const target = gliding ? player.position + 1 : player.position
    return Math.min(1, Math.max(0, target / player.duration))
  })

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
  <div class="fill" class:gliding style="transform: scaleX({fraction})"></div>
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
  .fill.gliding {
    transition: transform 1s linear;
  }
</style>
