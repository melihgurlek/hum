<script lang="ts">
  import { api, artistLine, fmtTime, img } from '../api'
  import { player, ui, setQuality, toggleLike } from '../state.svelte'
  import Icon from './Icon.svelte'
  import Progress from './Progress.svelte'

  let volume = $state(100)
</script>

<footer class="bar" aria-label="Player">
  <div class="left">
    {#if player.current}
      <button class="np" onclick={() => (ui.nowPlayingOpen = true)} aria-label="Open now playing">
        {#if player.current.thumb}
          <img class="cover art" src={img(player.current.thumb)} alt="" width="56" height="56" />
        {:else}
          <div class="cover art"></div>
        {/if}
        <span class="text">
          <span class="title ellipsis">{player.current.title}</span>
          <span class="sub ellipsis">{artistLine(player.current)}</span>
        </span>
      </button>
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
        <Icon name="leaf" size={14} /> Data saver
      </button>
    {/if}
    <button class="icon-btn" aria-label="Up next" onclick={() => (ui.nowPlayingOpen = true)}>
      <Icon name="queue" size={18} />
    </button>
    <Icon name="volume" size={18} />
    <input
      class="vol"
      type="range"
      min="0"
      max="100"
      bind:value={volume}
      oninput={() => api.setVolume(volume)}
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
  .np {
    display: flex;
    align-items: center;
    gap: 12px;
    min-width: 0;
    text-align: left;
    border-radius: 8px;
  }
  .art {
    width: 56px;
    height: 56px;
    border-radius: 8px;
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
