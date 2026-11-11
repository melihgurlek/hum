<script lang="ts">
  import { api, artistLine, fmtTime, img, type TrackNote } from '../api'
  import { player, ui, setAutoplay, toggleLike, reason } from '../state.svelte'
  import Icon from '../components/Icon.svelte'
  import Progress from '../components/Progress.svelte'

  let tab = $state<'next' | 'lyrics'>('next')

  // Why each song is in Up next, from your listening history.
  let notes = $state<Record<string, TrackNote>>({})
  $effect(() => {
    const ids = player.upNext.slice(0, 50).map((t) => t.id)
    if (!ids.length) return
    api.trackNotes(ids).then((n) => (notes = n)).catch(() => {})
  })
  const why = (id: string) => reason(notes[id])

  function close() {
    ui.nowPlayingOpen = false
  }
</script>

<svelte:window onkeydown={(e) => e.key === 'Escape' && close()} />

<div class="np" role="dialog" aria-label="Now playing">
  <header>
    <button class="round" onclick={close} aria-label="Close now playing"><Icon name="chevronDown" /></button>
    <div class="from">
      {#if ui.autoplay}Playing with <strong>autoplay radio</strong>{:else}Playing your queue{/if}
    </div>
    <span class="spacer"></span>
  </header>

  {#if player.current}
    {@const t = player.current}
    <div class="body">
      <section class="left" aria-label="Song">
        {#if t.cover}
          <img class="art" src={img(t.cover)} alt={`${t.album ?? t.title} cover`} />
        {:else}
          <div class="art"></div>
        {/if}
        <div class="meta">
          <div class="titles">
            <h1>{t.title}</h1>
            <div class="artist">{artistLine(t)}</div>
            {#if t.album}<div class="album">{t.album}</div>{/if}
          </div>
          <button class="heart" class:on={player.liked} aria-pressed={player.liked}
            aria-label={player.liked ? 'Remove from Liked songs' : 'Add to Liked songs'} onclick={toggleLike}>
            <Icon name={player.liked ? 'heart' : 'heartOutline'} size={24} />
          </button>
        </div>
        <div class="progress">
          <Progress tone="np" />
          <div class="times"><span>{fmtTime(player.position)}</span><span>{fmtTime(player.duration)}</span></div>
        </div>
        <div class="controls">
          <button class="ctl" aria-label="Previous" onclick={() => api.previous()}><Icon name="prev" size={28} /></button>
          <button class="big" aria-label={player.paused ? 'Play' : 'Pause'} onclick={() => api.togglePause()}>
            <Icon name={player.paused ? 'play' : 'pause'} size={30} />
          </button>
          <button class="ctl" aria-label="Next" onclick={() => api.next()}><Icon name="next" size={28} /></button>
        </div>
      </section>

      <section class="panel" aria-label="Up next and lyrics">
        <div class="tabs" role="tablist">
          <button role="tab" aria-selected={tab === 'next'} class:sel={tab === 'next'} onclick={() => (tab = 'next')}>Up next</button>
          <button role="tab" aria-selected={tab === 'lyrics'} class:sel={tab === 'lyrics'} onclick={() => (tab = 'lyrics')}>Lyrics</button>
          <span class="spacer"></span>
          <label class="auto">
            Autoplay radio
            <input type="checkbox" checked={ui.autoplay} onchange={(e) => setAutoplay(e.currentTarget.checked)} />
          </label>
        </div>

        {#if tab === 'next'}
          <div class="queue scroll">
            {#each player.upNext.slice(0, 50) as q, i (q.id)}
              <button class="qrow" onclick={() => api.jump(player.index + 1 + i)}>
                {#if q.thumb}<img class="qart" src={img(q.thumb)} alt="" loading="lazy" />{:else}<div class="qart"></div>{/if}
                <span class="qtext"><span class="ellipsis qt">{q.title}</span><span class="ellipsis qs">{artistLine(q)}</span></span>
                <span class="why">{why(q.id)}</span>
                <span class="qd">{fmtTime(q.duration)}</span>
              </button>
            {:else}
              <p class="empty">
                {#if ui.autoplay}Finding songs like this one…{:else}Nothing queued. Turn on autoplay radio to keep the music going.{/if}
              </p>
            {/each}
          </div>
        {:else}
          <p class="empty">Synced lyrics are coming in a later version.</p>
        {/if}
      </section>
    </div>
  {:else}
    <p class="empty">Nothing is playing.</p>
  {/if}
</div>

<style>
  .np {
    position: absolute;
    inset: 0;
    z-index: 10;
    display: flex;
    flex-direction: column;
    color: var(--np-text);
    background: radial-gradient(1100px 700px at 22% 30%, var(--np-bg, var(--np-bg-default)) 0%, var(--np-bg-2, var(--np-bg-2-default)) 100%);
    transition: background 0.4s;
  }
  header {
    display: flex;
    align-items: center;
    gap: 12px;
    padding: 16px 24px;
  }
  .round {
    width: 44px;
    height: 44px;
    border-radius: 50%;
    background: color-mix(in srgb, black 22%, transparent);
    display: flex;
    align-items: center;
    justify-content: center;
  }
  .from {
    font-size: 13px;
    color: var(--np-muted);
  }
  .from strong {
    color: var(--np-text);
    font-weight: 600;
  }
  .spacer {
    flex: 1;
  }
  .body {
    flex: 1;
    min-height: 0;
    display: flex;
    flex-wrap: wrap;
    gap: 48px;
    padding: 4px 56px 32px;
    align-items: stretch;
  }
  .left {
    flex: 1 1 340px;
    max-width: 440px;
    display: flex;
    flex-direction: column;
    gap: 20px;
    justify-content: center;
  }
  .art {
    width: 100%;
    aspect-ratio: 1;
    border-radius: 16px;
    object-fit: cover;
    background: color-mix(in srgb, black 25%, transparent);
    box-shadow: 0 24px 48px rgba(0, 0, 0, 0.35);
  }
  .meta {
    display: flex;
    align-items: flex-start;
    gap: 12px;
  }
  .titles {
    flex: 1;
    min-width: 0;
  }
  .heart {
    width: 44px;
    height: 44px;
    border-radius: 50%;
    display: flex;
    align-items: center;
    justify-content: center;
    color: var(--np-muted);
    flex: none;
  }
  .heart.on {
    color: var(--np-text);
  }
  h1 {
    font-size: 30px;
    font-weight: 800;
    line-height: 1.1;
  }
  .artist {
    font-size: 18px;
    color: var(--np-muted);
    margin-top: 4px;
  }
  .album {
    font-size: 14px;
    color: var(--np-muted);
  }
  .progress {
    display: flex;
    flex-direction: column;
    gap: 4px;
  }
  .times {
    display: flex;
    justify-content: space-between;
    font-size: 13px;
    color: var(--np-muted);
    font-variant-numeric: tabular-nums;
  }
  .controls {
    display: flex;
    align-items: center;
    justify-content: center;
    gap: 28px;
  }
  .ctl {
    width: 52px;
    height: 52px;
    display: flex;
    align-items: center;
    justify-content: center;
  }
  .big {
    width: 72px;
    height: 72px;
    border-radius: 50%;
    background: var(--np-text);
    color: var(--np-bg-2, var(--np-bg-2-default));
    display: flex;
    align-items: center;
    justify-content: center;
  }
  .panel {
    flex: 999 1 420px;
    min-width: 0;
    min-height: 0;
    display: flex;
    flex-direction: column;
    gap: 14px;
    background: color-mix(in srgb, black 20%, transparent);
    border-radius: 20px;
    padding: 18px 20px;
    max-height: calc(100vh - 160px);
  }
  :global([data-theme='light']) .panel {
    background: color-mix(in srgb, white 45%, transparent);
  }
  .tabs {
    display: flex;
    align-items: center;
    gap: 6px;
  }
  .tabs button {
    height: 40px;
    padding: 0 18px;
    border-radius: 999px;
    font-weight: 600;
    color: var(--np-muted);
  }
  .tabs button.sel {
    background: var(--np-text);
    color: var(--np-bg-2, var(--np-bg-2-default));
  }
  .auto {
    display: flex;
    align-items: center;
    gap: 8px;
    font-size: 13px;
    color: var(--np-muted);
    cursor: pointer;
  }
  .auto input {
    accent-color: var(--accent, var(--accent-default));
    width: 18px;
    height: 18px;
  }
  .queue {
    flex: 1;
    min-height: 0;
    display: flex;
    flex-direction: column;
    gap: 2px;
  }
  .qrow {
    display: flex;
    align-items: center;
    gap: 12px;
    padding: 6px 8px;
    border-radius: 10px;
    text-align: left;
    min-height: 52px;
  }
  .qrow:hover {
    background: color-mix(in srgb, var(--np-text) 8%, transparent);
  }
  .qart {
    width: 40px;
    height: 40px;
    border-radius: 6px;
    object-fit: cover;
    flex: none;
    background: color-mix(in srgb, black 25%, transparent);
  }
  .qtext {
    flex: 1;
    min-width: 0;
    display: flex;
    flex-direction: column;
  }
  .qt {
    font-weight: 600;
  }
  .qs,
  .qd,
  .why {
    font-size: 13px;
    color: var(--np-muted);
  }
  .why {
    font-weight: 600;
    white-space: nowrap;
  }
  .qd {
    font-variant-numeric: tabular-nums;
    width: 40px;
    text-align: right;
  }
  .empty {
    color: var(--np-muted);
    padding: 24px;
    font-size: 15px;
  }
</style>
