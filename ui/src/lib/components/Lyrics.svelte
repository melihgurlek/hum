<script lang="ts">
  import { api, type Lyrics } from '../api'
  import { player } from '../state.svelte'

  let lyrics = $state<Lyrics | null>(null)
  let status = $state<'loading' | 'ready' | 'none' | 'error'>('loading')
  let active = $state(-1)
  let box: HTMLDivElement | undefined = $state()
  let followPausedUntil = 0

  // Load when the song changes.
  $effect(() => {
    const t = player.current
    if (!t) return
    lyrics = null
    active = -1
    status = 'loading'
    api
      .lyrics(t)
      .then((l) => {
        if (player.current?.id !== t.id) return
        lyrics = l
        status = l ? 'ready' : 'none'
      })
      .catch(() => {
        if (player.current?.id === t.id) status = 'error'
      })
  })

  const lines = $derived(lyrics?.lines ?? null)

  // Follow the song: estimate the position between the engine's once-a-second reports and only
  // touch the DOM when the active line changes. No loop at all while paused.
  $effect(() => {
    if (!lines || player.paused) {
      if (lines) active = indexAt(player.position)
      return
    }
    let raf = 0
    const tick = () => {
      const pos = player.position + (performance.now() - player.positionAt) / 1000
      const i = indexAt(pos)
      if (i !== active) active = i
      raf = requestAnimationFrame(tick)
    }
    raf = requestAnimationFrame(tick)
    return () => cancelAnimationFrame(raf)
  })

  function indexAt(pos: number): number {
    if (!lines) return -1
    // Show a line slightly before it's sung, so reading keeps up.
    const p = pos + 0.25
    let lo = 0,
      hi = lines.length - 1,
      ans = -1
    while (lo <= hi) {
      const mid = (lo + hi) >> 1
      if (lines[mid].at <= p) {
        ans = mid
        lo = mid + 1
      } else hi = mid - 1
    }
    return ans
  }

  // Keep the active line in the middle, unless you've just scrolled yourself.
  $effect(() => {
    const i = active
    if (i < 0 || !box || performance.now() < followPausedUntil) return
    const el = box.querySelector<HTMLElement>(`[data-i="${i}"]`)
    el?.scrollIntoView({ block: 'center', behavior: 'smooth' })
  })

  function userScrolled() {
    followPausedUntil = performance.now() + 4000
  }
</script>

<div class="lyrics scroll" role="region" aria-label="Lyrics" bind:this={box} onwheel={userScrolled} ontouchmove={userScrolled}>
  {#if status === 'loading'}
    <p class="note">Looking for lyrics…</p>
  {:else if status === 'error'}
    <p class="note">Couldn't load lyrics. Check your connection.</p>
  {:else if status === 'none' || !lyrics}
    <p class="note">No lyrics found for this song.</p>
  {:else if lines}
    <div class="pad"></div>
    {#each lines as line, i (i)}
      <button class="line" class:active={i === active} class:past={i < active} data-i={i}
        onclick={() => api.seek(line.at)}>
        {line.text || '♪'}
      </button>
    {/each}
    <div class="pad"></div>
    <p class="credit">Lyrics from {lyrics.source}</p>
  {:else}
    <p class="plain">{lyrics.plain}</p>
    <p class="credit">Lyrics from {lyrics.source}. These aren't synced to the song.</p>
  {/if}
</div>

<style>
  .lyrics {
    flex: 1;
    min-height: 0;
    display: flex;
    flex-direction: column;
    gap: 6px;
    padding: 0 8px;
    user-select: text;
  }
  .pad {
    flex: none;
    height: 30%;
  }
  .line {
    text-align: left;
    font-size: 26px;
    font-weight: 700;
    line-height: 1.25;
    letter-spacing: -0.01em;
    color: var(--np-muted);
    opacity: 0.55;
    padding: 6px 8px;
    border-radius: 10px;
    transition:
      opacity 0.25s,
      color 0.25s;
  }
  .line:hover {
    opacity: 0.85;
  }
  .line.past {
    opacity: 0.4;
  }
  .line.active {
    color: var(--np-text);
    opacity: 1;
  }
  .plain {
    white-space: pre-line;
    font-size: 18px;
    line-height: 1.6;
    color: var(--np-text);
    margin: 8px;
  }
  .note,
  .credit {
    color: var(--np-muted);
    font-size: 13px;
    margin: 8px;
  }
  .note {
    font-size: 15px;
    padding: 16px 0;
  }
</style>
