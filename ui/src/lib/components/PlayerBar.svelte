<script lang="ts">
  import { api, artistLinks, fmtTime, img } from '../api'
  import { openNowPlaying, player, ui, setQuality, setSleep, sleep, toggleLike } from '../state.svelte'
  import Icon from './Icon.svelte'
  import ArtistLinks from './ArtistLinks.svelte'
  import Progress from './Progress.svelte'

  let sleepOpen = $state(false)
  // Re-read the clock twice a minute so the minutes left stay current.
  let now = $state(Date.now())
  $effect(() => {
    if (!sleep.until) return
    const t = setInterval(() => (now = Date.now()), 30_000)
    return () => clearInterval(t)
  })
  const sleepLabel = $derived(
    sleep.endOfSong ? 'End of song' : sleep.until ? `${Math.max(1, Math.ceil((sleep.until - now) / 60_000))} min` : null,
  )
  function pick(c: number | 'song' | null) {
    setSleep(c)
    now = Date.now()
    sleepOpen = false
  }
</script>

<footer class="bar" class:tinted={ui.nowPlayingOpen} aria-label="Player">
  <!-- Phone: a thin progress line along the top of the mini player. -->
  {#if player.current && player.duration}<span class="mini-progress" style:width={`${Math.min(100, (player.position / player.duration) * 100)}%`}></span>{/if}
  <div class="left">
    {#if player.current}
      <div class="np">
        {#if player.current.thumb}
          <img class="cover art" src={img(player.current.thumb)} alt="" width="56" height="56" />
        {:else}
          <div class="cover art"></div>
        {/if}
        <span class="text">
          <button class="open title ellipsis" onclick={openNowPlaying} aria-label={`Open now playing: ${player.current.title}`}>{player.current.title}</button>
          <span class="sub ellipsis"><ArtistLinks artists={artistLinks(player.current)} /></span>
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
    <div class="sleep">
      <button class="icon-btn" class:on={sleepLabel} aria-label="Sleep timer" aria-expanded={sleepOpen}
        title={sleepLabel ? `Sleep timer: ${sleepLabel}` : 'Sleep timer'} onclick={() => (sleepOpen = !sleepOpen)}>
        <Icon name="moon" size={18} />{#if sleepLabel}<span class="left-t">{sleepLabel}</span>{/if}
      </button>
      {#if sleepOpen}
        <button class="backdrop" aria-label="Close" onclick={() => (sleepOpen = false)}></button>
        <div class="menu" role="menu" aria-label="Sleep timer">
          <div class="mt">Pause after</div>
          {#each [15, 30, 45, 60] as m}<button role="menuitem" onclick={() => pick(m)}>{m} minutes</button>{/each}
          <button role="menuitem" onclick={() => pick('song')}>End of this song</button>
          {#if sleepLabel}<button role="menuitem" class="off" onclick={() => pick(null)}>Turn off</button>{/if}
        </div>
      {/if}
    </div>
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
  .sleep {
    position: relative;
  }
  .sleep .icon-btn {
    width: auto;
    min-width: 36px;
    gap: 6px;
    padding: 0 8px;
  }
  .left-t {
    font-size: 12px;
    font-weight: 700;
    white-space: nowrap;
  }
  .backdrop {
    position: fixed;
    inset: 0;
    z-index: 29;
    cursor: default;
  }
  .menu {
    position: absolute;
    right: 0;
    bottom: calc(100% + 10px);
    z-index: 30;
    width: 200px;
    padding: 6px;
    border-radius: var(--r-lg);
    background: var(--raised);
    border: 1px solid var(--line);
    box-shadow: 0 12px 32px rgb(0 0 0 / 0.35);
    display: flex;
    flex-direction: column;
    color: var(--text);
  }
  .mt {
    font-size: 12px;
    font-weight: 700;
    color: var(--muted);
    padding: 6px 10px 4px;
  }
  .menu button {
    text-align: left;
    padding: 0 10px;
    min-height: 36px;
    border-radius: var(--r-md);
    font-weight: 600;
  }
  .menu button:hover {
    background: color-mix(in srgb, var(--text) 8%, transparent);
  }
  .menu .off {
    color: var(--muted);
  }
  .np {
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
    .controls .icon-btn {
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
</style>
