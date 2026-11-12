<script lang="ts">
  import { api, artistLine, artistLinks, fmtTime, img } from '../api'
  import { layout, openNowPlaying, player, ui, setQuality, toggleLike } from '../state.svelte'
  import Icon from './Icon.svelte'
  import ArtistLinks from './ArtistLinks.svelte'
  import Progress from './Progress.svelte'
  import SleepTimer from './SleepTimer.svelte'
  import RepeatButton from './RepeatButton.svelte'
  import ShuffleButton from './ShuffleButton.svelte'

  // Phone mini player: swipe the song sideways to skip (left for next, right for previous),
  // or up to open Now Playing. The first 10px decide the direction, so taps still work.
  let dx = $state(0)
  let settling = $state(false)
  let start: { x: number; y: number; t: number } | null = null
  let axis: 'x' | 'y' | null = null
  let swiped = false
  function down(e: PointerEvent) {
    if (!layout.phone || e.pointerType === 'mouse' || !player.current) return
    // The screen's edges belong to Android's back gesture.
    if (e.clientX < 24 || e.clientX > innerWidth - 24) return
    start = { x: e.clientX, y: e.clientY, t: performance.now() }
    axis = null
    swiped = false
  }
  function move(e: PointerEvent) {
    if (!start) return
    const mx = e.clientX - start.x
    const my = e.clientY - start.y
    if (!axis && Math.hypot(mx, my) > 10) {
      axis = Math.abs(mx) > Math.abs(my) ? 'x' : 'y'
      try {
        ;(e.currentTarget as HTMLElement).setPointerCapture(e.pointerId)
      } catch {}
    }
    if (axis === 'x') {
      settling = false
      dx = mx
    }
  }
  function up(e: PointerEvent) {
    if (!start) return
    const mx = e.clientX - start.x
    const my = e.clientY - start.y
    const fast = Math.abs(mx) / Math.max(1, performance.now() - start.t) > 0.5
    const width = (e.currentTarget as HTMLElement).clientWidth
    if (axis) swiped = true
    if (axis === 'x' && (Math.abs(mx) > width * 0.3 || fast)) {
      // Slide the song off that side, change track, and bring the new one in from the other.
      // The new song is shown as it slides in, before the engine confirms it, so there's no
      // empty beat between the two. (Previous more than 3 s in restarts this song, as the
      // button does.)
      const dir = Math.sign(mx)
      settling = true
      dx = dir * width
      setTimeout(() => {
        const target = dir < 0 ? player.queue[player.index + 1] : player.position > 3 ? player.current : player.queue[player.index - 1]
        if (target) player.current = target
        if (dir < 0) api.next()
        else api.previous()
        settling = false
        dx = -dir * width * 0.25
        requestAnimationFrame(() => requestAnimationFrame(() => ((settling = true), (dx = 0))))
      }, 110)
    } else {
      if (axis === 'y' && my < -40) openNowPlaying()
      settling = true
      dx = 0
    }
    start = null
    axis = null
  }
  function cancel() {
    start = null
    axis = null
    settling = true
    dx = 0
  }
  // A swipe ends with a click on whatever it started on; don't let that open Now Playing.
  function click(e: MouseEvent) {
    if (swiped) {
      e.stopPropagation()
      e.preventDefault()
      swiped = false
    }
  }
</script>

<footer class="bar" class:tinted={ui.nowPlayingOpen} aria-label="Player"
  onpointerdown={down} onpointermove={move} onpointerup={up} onpointercancel={cancel} onclickcapture={click}>
  <!-- Phone: a thin progress line along the top of the mini player. -->
  {#if player.current && player.duration}<span class="mini-progress" style:width={`${Math.min(100, (player.position / player.duration) * 100)}%`}></span>{/if}
  <div class="left" class:settling style:transform={dx ? `translateX(${dx}px)` : undefined}
    style:opacity={dx ? Math.max(0.2, 1 - Math.abs(dx) / 300) : undefined}>
    {#if player.current}
      <div class="np">
        {#if player.current.thumb}
          <img class="cover art" src={img(player.current.thumb)} alt="" width="56" height="56" />
        {:else}
          <div class="cover art"></div>
        {/if}
        <span class="text">
          <button class="open title ellipsis" onclick={openNowPlaying} aria-label={`Open now playing: ${player.current.title}`}>{player.current.title}</button>
          <!-- Phone: plain names. Links there were easy to hit by accident; the whole bar opens
               Now Playing, where the artists are links. -->
          <span class="sub ellipsis">{#if layout.phone}{artistLine(player.current)}{:else}<ArtistLinks artists={artistLinks(player.current)} />{/if}</span>
        </span>
      </div>
      <button class="icon-btn" class:on={player.liked} aria-pressed={player.liked}
        aria-label={player.liked ? 'Remove from Liked songs' : 'Add to Liked songs'} onclick={toggleLike}>
        <Icon name={player.liked ? 'heart' : 'heartOutline'} size={20} />
      </button>
    {:else}
      <span class="muted">Pick a song to start</span>
    {/if}
  </div>

  <div class="center">
    <div class="controls">
      <ShuffleButton size={18} />
      <button class="icon-btn" aria-label="Previous" onclick={() => api.previous()} disabled={!player.current}>
        <Icon name="prev" />
      </button>
      <button
        class="play"
        aria-label={player.paused ? 'Play' : 'Pause'}
        onclick={() => api.togglePause()}
        disabled={!player.current}
      >
        {#if player.loading}
          <span class="spinner" aria-hidden="true"></span>
        {:else}
          <Icon name={player.paused ? 'play' : 'pause'} />
        {/if}
      </button>
      <button class="icon-btn" aria-label="Next" onclick={() => api.next()} disabled={!player.current}>
        <Icon name="next" />
      </button>
      <RepeatButton size={18} />
    </div>
    <div class="time">
      <span>{fmtTime(player.position)}</span>
      <Progress />
      <span>{fmtTime(player.duration)}</span>
    </div>
  </div>

  <div class="right">
    {#if ui.quality === 'data_saver'}
      <button class="pill" onclick={() => setQuality('auto')} title="Streaming at about 50 kbps. Click to switch back to Auto.">
        <Icon name="leaf" size={14} /><span class="trim">Data saver</span>
      </button>
    {/if}
    <SleepTimer />
    <button class="icon-btn" aria-label="Up next" onclick={openNowPlaying}>
      <Icon name="queue" size={18} />
    </button>
    <Icon name="volume" size={18} />
    <input
      class="vol"
      type="range"
      min="0"
      max="100"
      bind:value={player.volume}
      oninput={() => api.setVolume(player.volume)}
      aria-label="Volume"
    />
  </div>
</footer>

<style>
  .bar {
    display: grid;
    grid-template-columns: minmax(180px, 1fr) minmax(300px, 2fr) minmax(180px, 1fr);
    align-items: center;
    gap: 24px;
    padding: 10px 20px;
    background: var(--bar);
    border-top: 1px solid var(--line);
    transition: background-color 0.4s, border-color 0.4s;
  }
  /* Under Now Playing the bar is part of that screen: it takes the cover's deep shade (the
     background's outer color) and the screen's text colors. The sidebar stays neutral. */
  .bar.tinted {
    --text: var(--np-text);
    --muted: var(--np-muted);
    --line: color-mix(in srgb, var(--np-text) 14%, transparent);
    --raised: color-mix(in srgb, var(--np-text) 10%, transparent);
    --raised-2: color-mix(in srgb, var(--np-text) 16%, transparent);
    color: var(--text);
    background: var(--np-bg-2, var(--np-bg-2-default));
  }
  .left,
  .right {
    display: flex;
    align-items: center;
    min-width: 0;
  }
  .right {
    justify-content: flex-end;
    gap: 4px;
    color: var(--muted);
  }
  /* The song fills its column, so the heart stays in one place whatever the title's length. */
  .np {
    flex: 1;
    position: relative;
    display: flex;
    align-items: center;
    gap: 12px;
    min-width: 0;
    border-radius: var(--r-sm);
  }
  /* The title button covers the whole cover-and-text block; artist links sit above it. */
  .open {
    display: block;
    max-width: 100%;
    padding: 0;
    text-align: left;
    font: inherit;
    color: inherit;
  }
  .open::after {
    content: '';
    position: absolute;
    inset: 0;
    border-radius: var(--r-sm);
  }
  .open:focus-visible {
    outline: none;
  }
  .open:focus-visible::after {
    outline: 2px solid var(--accent, var(--accent-default));
  }
  .art {
    width: 56px;
    height: 56px;
    border-radius: var(--r-sm);
    flex: none;
  }
  .text {
    display: flex;
    flex-direction: column;
    min-width: 0;
  }
  .title {
    font-weight: 700;
  }
  .sub {
    color: var(--muted);
    font-size: 13px;
  }
  .center {
    display: flex;
    flex-direction: column;
    align-items: center;
    gap: 2px;
  }
  .controls {
    display: flex;
    align-items: center;
    gap: 8px;
  }
  .controls .icon-btn {
    color: var(--text);
  }
  .play {
    width: 48px;
    height: 48px;
    border-radius: 50%;
    background: var(--text);
    color: var(--bg);
    display: flex;
    align-items: center;
    justify-content: center;
  }
  button:disabled {
    opacity: 0.4;
    cursor: default;
  }
  .time {
    width: 100%;
    max-width: 560px;
    display: flex;
    align-items: center;
    gap: 10px;
    font-size: 12px;
    color: var(--muted);
    font-variant-numeric: tabular-nums;
  }
  .pill {
    display: inline-flex;
    align-items: center;
    gap: 6px;
    height: 28px;
    padding: 0 10px;
    border-radius: 999px;
    background: var(--raised);
    font-size: 12px;
    font-weight: 600;
    color: var(--muted);
    margin-right: 6px;
  }
  .vol {
    width: 90px;
    accent-color: var(--muted);
  }
  .spinner {
    width: 18px;
    height: 18px;
    border-radius: 50%;
    border: 2px solid currentColor;
    border-right-color: transparent;
    animation: spin 0.7s linear infinite;
  }
  @keyframes spin {
    to {
      transform: rotate(360deg);
    }
  }
  .mini-progress {
    display: none;
  }
  /* Phone: a mini player. Cover, title and artist open Now Playing; one play button; the
     phone's own buttons do volume, and Now Playing has the rest. */
  @media (max-width: 640px) {
    .bar {
      position: relative;
      gap: 12px;
      padding: 8px 8px 8px 12px;
      border-top: 0;
    }
    .bar.tinted {
      display: none;
    }
    /* The bar handles its own swipes (nothing in it scrolls). */
    .bar {
      touch-action: none;
    }
    .left.settling {
      transition: transform 0.11s ease-out, opacity 0.11s ease-out;
    }
    .mini-progress {
      display: block;
      position: absolute;
      left: 0;
      top: 0;
      height: 2px;
      background: var(--accent, var(--accent-default));
      transition: width 1s linear;
    }
    .np :global(.cover) {
      width: 44px;
      height: 44px;
    }

    .controls :global(.icon-btn) {
      display: none;
    }
    .play {
      width: 44px;
      height: 44px;
    }
  }
  @media (max-width: 860px) {
    .bar {
      grid-template-columns: 1fr auto;
    }
    .right {
      display: none;
    }
    .time {
      display: none;
    }
  }
  @media (hover: none) {
    .open:active {
      box-shadow: none;
    }
    .open::after {
      transition: background-color 0.25s;
    }
    .open:active::after {
      background: color-mix(in srgb, var(--text) 10%, transparent);
      transition: none;
    }
  }
</style>
