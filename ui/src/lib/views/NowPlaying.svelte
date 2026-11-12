<script lang="ts">
  import { api, artistLine, artistLinks, fmtTime, img, type TrackNote } from '../api'
  import { player, ui, setAutoplay, toggleLike, reason, openAddMenu, openAlbum, prefetchOnHover } from '../state.svelte'
  import ArtistLinks from '../components/ArtistLinks.svelte'
  import Icon from '../components/Icon.svelte'
  import Progress from '../components/Progress.svelte'
  import Lyrics from '../components/Lyrics.svelte'
  import SleepTimer from '../components/SleepTimer.svelte'

  // Remember which tab you left open.
  let tab = $state<'next' | 'lyrics'>((localStorage.getItem('hum.npTab') as 'next' | 'lyrics') ?? 'lyrics')
  $effect(() => {
    try {
      localStorage.setItem('hum.npTab', tab)
    } catch {}
  })

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

  // Phone: Up next and lyrics sit below the controls; this button scrolls down to them.
  let panel = $state<HTMLElement>()
  const showPanel = () => panel?.scrollIntoView({ behavior: 'smooth', block: 'start' })
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
            <div class="artist"><ArtistLinks artists={artistLinks(t)} /></div>
            {#if t.album}
              {#if t.album_id}<button class="album album-link" onclick={() => openAlbum(t.album_id)}>{t.album}</button>
              {:else}<div class="album">{t.album}</div>{/if}
            {/if}
          </div>
          <button class="heart" class:on={player.liked} aria-pressed={player.liked}
            aria-label={player.liked ? 'Remove from Liked songs' : 'Add to Liked songs'} onclick={toggleLike}>
            <Icon name={player.liked ? 'heart' : 'heartOutline'} size={24} />
          </button>
          <button class="heart" aria-label="Add to playlist" title="Add to playlist" onclick={(e) => openAddMenu([t], e)}>
            <Icon name="plus" size={24} />
          </button>
        </div>
        <div class="progress">
          <Progress tone="np" />
          <div class="times"><span>{fmtTime(player.position)}</span><span>{fmtTime(player.duration)}</span></div>
        </div>
        <div class="controls">
          <!-- Phone: the player bar is hidden here, so its sleep timer moves into this row. -->
          <span class="phone-only side-ctl"><SleepTimer size={22} align="left" /></span>
          <button class="ctl" aria-label="Previous" onclick={() => api.previous()}><Icon name="prev" size={28} /></button>
          <button class="big" aria-label={player.paused ? 'Play' : 'Pause'} onclick={() => api.togglePause()}>
            <Icon name={player.paused ? 'play' : 'pause'} size={30} />
          </button>
          <button class="ctl" aria-label="Next" onclick={() => api.next()}><Icon name="next" size={28} /></button>
          <span class="phone-only side-ctl right"><button class="icon-btn" aria-label="Show up next and lyrics" onclick={showPanel}><Icon name="queue" size={22} /></button></span>
        </div>
      </section>

      <section class="panel" bind:this={panel} aria-label="Up next and lyrics">
        <div class="tabs" role="tablist">
          <button role="tab" aria-selected={tab === 'next'} class:sel={tab === 'next'} onclick={() => (tab = 'next')}><span class="trim">Up next</span></button>
          <button role="tab" aria-selected={tab === 'lyrics'} class:sel={tab === 'lyrics'} onclick={() => (tab = 'lyrics')}><span class="trim">Lyrics</span></button>
          <span class="spacer"></span>
          <label class="auto">
            Autoplay radio
            <input type="checkbox" checked={ui.autoplay} onchange={(e) => setAutoplay(e.currentTarget.checked)} />
          </label>
        </div>

        {#if tab === 'next'}
          <div class="queue scroll">
            {#each player.upNext.slice(0, 50) as q, i (q.id)}
              <button class="qrow" use:prefetchOnHover={q.id} onclick={() => api.jump(player.index + 1 + i)}>
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
          <Lyrics />
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
  /* Two columns that fill exactly the height Now Playing has, so the panel ends above the
     player bar however tall the window is. (A wrapping flex row sized the panel by its
     content plus a 100vh guess, and it ran ~40px under the bar.) */
  .body {
    flex: 1;
    min-height: 0;
    display: grid;
    grid-template-columns: minmax(280px, 440px) minmax(0, 1fr);
    gap: 48px;
    padding: 4px 56px 32px;
  }
  .left {
    min-height: 0;
    display: flex;
    flex-direction: column;
    gap: 20px;
    justify-content: safe center;
  }
  /* The cover gives way first when the window is short: it takes the height left after the
     title and controls (its width follows), and never more than the column's width. */
  .art {
    flex: 0 1 440px;
    min-height: 96px;
    width: auto;
    max-width: 100%;
    align-self: flex-start;
    aspect-ratio: 1;
    border-radius: var(--r-lg);
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
  .album-link {
    display: block;
    padding: 0;
    text-align: left;
    font: inherit;
    font-size: 14px;
  }
  .album-link:hover,
  .album-link:focus-visible {
    text-decoration: underline;
  }
  .progress {
    display: flex;
    flex-direction: column;
    gap: 4px;
  }
  /* The column's 20px gap is right around the cover, too loose between the titles and the bar. */
  .meta + .progress {
    margin-top: -8px;
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
    border-radius: var(--r-lg);
    padding: 18px 20px;
  }
  /* Narrow windows: stack, and let the whole screen scroll instead. */
  @media (max-width: 900px) {
    /* A flex column, not a one-column grid: with min-height: 0 on the items, the grid's rows
       shrank to fit the screen and the panel slid over the cover. */
    .body {
      display: flex;
      flex-direction: column;
      overflow-y: auto;
      padding: 4px 24px 24px;
      gap: 24px;
    }
    .left {
      flex: none;
      max-width: 440px;
    }
    .panel {
      flex: none;
      height: 70vh;
    }
  }
  .phone-only {
    display: none;
  }
  /* Phone: Now Playing is its own full screen over the tab bar, one scrolling column: cover,
     song, progress, transport with the sleep timer, then Up next and lyrics below the fold. */
  @media (max-width: 640px) {
    .np {
      position: fixed;
      z-index: 40;
      padding-bottom: env(safe-area-inset-bottom);
    }
    header {
      padding: calc(8px + env(safe-area-inset-top)) 16px 4px;
    }
    .body {
      padding: 8px 24px 24px;
      gap: 32px;
    }
    .left {
      max-width: none;
      justify-content: flex-start;
    }
    .art {
      flex: none;
      width: 100%;
      height: auto;
      min-height: 0;
      align-self: center;
      box-shadow: 0 16px 36px rgba(0, 0, 0, 0.35);
    }
    h1 {
      font-size: 24px;
    }
    .artist {
      font-size: 16px;
    }
    .meta .heart {
      margin-right: -10px;
    }
    .meta .heart + .heart {
      margin-left: -12px;
    }
    .controls {
      gap: 0;
      justify-content: space-between;
    }
    .phone-only {
      display: flex;
    }
    .side-ctl {
      width: 64px;
      color: var(--np-muted);
      --text: var(--np-text);
      --muted: var(--np-muted);
    }
    .side-ctl.right {
      justify-content: flex-end;
    }
    .side-ctl :global(.icon-btn) {
      color: var(--np-muted);
    }
    .side-ctl :global(.icon-btn.on) {
      color: var(--np-text);
    }
    .panel {
      height: calc(100dvh - 120px);
      margin: 0 -12px;
      padding: 14px 12px;
      scroll-margin-top: 12px;
    }
    .tabs {
      flex-wrap: wrap;
    }
    .qd {
      display: none;
    }
    .why {
      max-width: 38%;
      overflow: hidden;
      text-overflow: ellipsis;
    }
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
    border-radius: var(--r-md);
    text-align: left;
    min-height: 52px;
  }
  .qrow:hover {
    background: color-mix(in srgb, var(--np-text) 8%, transparent);
  }
  .qart {
    width: 40px;
    height: 40px;
    border-radius: var(--r-sm);
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
