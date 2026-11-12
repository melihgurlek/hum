<script lang="ts">
  // The fill moves once a second, on the engine's position reports. No CSS transition: a
  // transition restarted every second keeps the webview drawing 60 frames a second for the
  // whole song (~19 % of a core on Linux, ~28 % on Windows, vs ~2 % stepping).
  import { api } from '../api'
  import { player } from '../state.svelte'

  let { tone = 'bar' }: { tone?: 'bar' | 'np' } = $props()

  // While a finger drags along the bar, the fill follows it and the seek happens on release.
  let scrub = $state<number | null>(null)
  const fraction = $derived(
    scrub ?? (player.duration ? Math.min(1, Math.max(0, player.position / player.duration)) : 0),
  )

  let el: HTMLDivElement
  const at = (x: number) => {
    const r = el.getBoundingClientRect()
    return Math.min(1, Math.max(0, (x - r.left) / r.width))
  }
  // Show the new spot at once: while paused no position report comes until playback resumes.
  function seekTo(secs: number) {
    player.position = secs
    player.positionAt = performance.now()
    api.seek(secs)
  }
  function down(e: PointerEvent) {
    if (!player.duration) return
    try {
      el.setPointerCapture(e.pointerId)
    } catch {}
    scrub = at(e.clientX)
  }
  function move(e: PointerEvent) {
    if (scrub !== null) scrub = at(e.clientX)
  }
  function up(e: PointerEvent) {
    if (scrub === null) return
    seekTo(at(e.clientX) * player.duration)
    scrub = null
  }
  function key(e: KeyboardEvent) {
    if (e.key === 'ArrowRight') seekTo(Math.min(player.duration, player.position + 5))
    if (e.key === 'ArrowLeft') seekTo(Math.max(0, player.position - 5))
  }
</script>

<div
  class="track {tone}"
  bind:this={el}
  onpointerdown={down}
  onpointermove={move}
  onpointerup={up}
  onpointercancel={() => (scrub = null)}
  onkeydown={key}
  role="slider"
  tabindex="0"
  aria-label="Seek"
  aria-valuemin="0"
  aria-valuemax={Math.round(player.duration)}
  aria-valuenow={Math.round(player.position)}
>
  <div class="fill" style="transform: scaleX({fraction})"></div>
  {#if tone === 'np'}<div class="knob" class:held={scrub !== null} style:left={`${fraction * 100}%`}></div>{/if}
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
  .np {
    height: 24px;
    touch-action: none;
  }
  .np::before {
    background: color-mix(in srgb, var(--np-text) 22%, transparent);
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
  }
  .knob {
    position: absolute;
    top: 50%;
    width: 12px;
    height: 12px;
    margin: -6px 0 0 -6px;
    border-radius: 50%;
    background: var(--np-text);
    transition: transform 0.12s;
  }
  .knob.held {
    transform: scale(1.4);
  }
  .track:hover .fill {
    background: var(--accent, var(--accent-default));
  }
</style>
