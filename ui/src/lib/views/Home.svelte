<script lang="ts">
  import { api, artistLine, type HomeCard, type QuickPicks, type Track, img } from '../api'
  import { capSet, go, homeFeed, layout, playList, playRadio, playYtPlaylist, prefetchOnHover, quickPicksCache as cache, ui } from '../state.svelte'
  import AlbumCard from '../components/AlbumCard.svelte'
  import Icon from '../components/Icon.svelte'
  import TrackRow from '../components/TrackRow.svelte'

  // Quick picks: up to four seeds' radios (your latest plays and favorites, or your likes on a
  // fresh install) fused and ranked by your taste. Cached for the session, keyed by the latest
  // play, so returning to Home is instant.
  const key = $derived(ui.recent[0]?.id ?? '')
  let qp = $state<QuickPicks | null>(null)
  let loading = $state(false)
  $effect(() => {
    const k = key
    if (cache.has(k)) {
      qp = cache.get(k)!
      return
    }
    loading = true
    api
      .quickPicks()
      .then((r) => {
        capSet(cache, k, r, 10)
        if (key === k) qp = r
      })
      .catch(() => {})
      .finally(() => (loading = false))
  })
  const seed = $derived(ui.recent[0] ?? qp?.seeds[0] ?? null)
  const picks = $derived(qp?.picks ?? [])
  // Desktop shows the first 12 (about four rows) so the sections below stay in sight; phones
  // swipe through all of them four rows at a time.
  let allPicks = $state(false)
  const shownPicks = $derived(layout.phone || allPicks ? picks : picks.slice(0, 12))

  let forgotten = $state<{ track: Track; last_played: number }[]>([])
  $effect(() => {
    api.forgotten(8).then((f) => (forgotten = f)).catch(() => {})
  })
  const monthYear = (ts: number) =>
    new Date(ts * 1000).toLocaleDateString('en-GB', { month: 'long', year: 'numeric' })

  // Until something has been played here, Home shows YouTube Music's own home page (the
  // account's when signed in), so a first launch isn't empty.
  const fresh = $derived(ui.recent.length === 0)
  $effect(() => {
    if (!fresh || homeFeed.shelves) return
    api.homeFeed().then((s) => (homeFeed.shelves = s)).catch(() => {})
  })
  // One row of cards per shelf, as many as fit at 150px or wider with 20px gaps.
  let rowWidth = $state(0)
  const cols = $derived(Math.max(2, Math.floor((rowWidth + 20) / 170)))
  // Phone: two cards a row is too few, so shelves become carousels you swipe through.
  const shown = $derived(layout.phone ? 12 : cols)
  const asCard = (c: Extract<HomeCard, { type: 'playlist' }>) =>
    ({ id: c.id, title: c.title, kind: 'Playlist', year: null, artists: [], thumb: c.thumb })
</script>

<div class="page scroll">
  {#if seed}
    <div class="cta-row">
      <button class="cta" use:prefetchOnHover={seed.id} onclick={() => playRadio(seed)}>
        <span class="play"><Icon name="play" size={16} /></span>
        <span class="trim">Keep going with {seed.artists[0] ?? seed.title} radio</span>
      </button>
    </div>

    <section>
      <div class="head">
        <h2>{fresh ? 'From your liked songs' : 'Quick picks'}</h2>
        {#if picks.length}<button class="link" use:prefetchOnHover={picks[0]?.id} onclick={() => playList(picks, 0)}>Play all</button>{/if}
      </div>
      {#if loading && !picks.length}
        <p class="muted">Finding songs like {seed.title}…</p>
      {:else}
        <div class="picks">
          {#each shownPicks as t, i (t.id)}
            <TrackRow track={t} onplay={() => playList(picks, i)} />
          {/each}
        </div>
        {#if !layout.phone && picks.length > 12}
          <button class="link more" onclick={() => (allPicks = !allPicks)}>{allPicks ? 'Show fewer' : `Show all ${picks.length}`}</button>
        {/if}
      {/if}
    </section>

    {#if forgotten.length}
      <section>
        <div class="head">
          <h2>Haven't played these in a while</h2>
          <button class="link" use:prefetchOnHover={forgotten[0]?.track.id} onclick={() => playList(forgotten.map((f) => f.track), 0)}>Play all</button>
        </div>
        <div class="picks">
          {#each forgotten as f, i (f.track.id)}
            <TrackRow track={f.track} suffix={`, last played ${monthYear(f.last_played)}`}
              onplay={() => playList(forgotten.map((x) => x.track), i)} />
          {/each}
        </div>
      </section>
    {/if}

    {#if ui.recent.length}
      <section>
        <h2>Recently played</h2>
        <div class="tiles">
          {#each ui.recent.slice(0, 12) as t (t.id)}
            <button class="tile" use:prefetchOnHover={t.id} onclick={() => playRadio(t)} title={`Radio from ${t.title}`}>
              {#if t.cover}<img class="cover art" src={img(t.cover)} alt="" loading="lazy" />{:else}<div class="cover art"></div>{/if}
              <span class="ellipsis tt">{t.title}</span>
              <span class="ellipsis muted ts">{artistLine(t)}</span>
            </button>
          {/each}
        </div>
      </section>
    {/if}
  {:else if !loading}
    <section class="empty">
      <h1>Start with a song you like</h1>
      <p class="muted">hum keeps going with songs like it.</p>
      <button class="cta" onclick={() => go('search')}><span class="play"><Icon name="search" size={16} /></span><span class="trim">Search for a song</span></button>
    </section>
  {/if}

  {#if fresh && homeFeed.shelves}
    {#each homeFeed.shelves as shelf (shelf.title)}
      <section>
        <div class="head">
          <h2>{shelf.title}</h2>
          {#if shelf.songs.length}<button class="link" use:prefetchOnHover={shelf.songs[0]?.id} onclick={() => playList(shelf.songs, 0)}>Play all</button>{/if}
        </div>
        {#if shelf.songs.length}
          <div class="picks">
            {#each shelf.songs.slice(0, 8) as t, i (t.id)}
              <TrackRow track={t} onplay={() => playList(shelf.songs, i)} />
            {/each}
          </div>
        {/if}
        {#if shelf.cards.length}
          <div class="row" style:--cols={cols} bind:clientWidth={rowWidth}>
            {#each shelf.cards.slice(0, shown) as c (c.id)}
              {#if c.type === 'album'}
                <AlbumCard album={c} showArtist />
              {:else}
                <AlbumCard album={asCard(c)} subtitle={c.subtitle} onplay={() => playYtPlaylist(c.id)} onopen={() => playYtPlaylist(c.id)} />
              {/if}
            {/each}
          </div>
        {/if}
      </section>
    {/each}
  {/if}
</div>

<style>
  .page {
    height: 100%;
    padding: 28px 32px 40px;
    display: flex;
    flex-direction: column;
    gap: 28px;
  }
  h1 {
    font-size: 24px;
    font-weight: 700;
  }
  h2 {
    font-size: 18px;
    font-weight: 700;
  }
  .head {
    display: flex;
    justify-content: space-between;
    align-items: baseline;
    margin-bottom: 8px;
  }
  section > h2 {
    margin-bottom: 12px;
  }
  .link {
    color: var(--muted);
    font-weight: 600;
    min-height: 44px;
  }
  .link:hover {
    color: var(--text);
  }
  .cta-row {
    display: flex;
    gap: 10px;
    flex-wrap: wrap;
  }
  .cta {
    display: inline-flex;
    align-items: center;
    gap: 12px;
    height: 56px;
    padding: 0 22px 0 8px;
    border-radius: var(--r-lg);
    background: var(--accent, var(--accent-default));
    color: var(--on-accent);
    font-weight: 700;
    font-size: 15px;
    transition: background 0.4s;
  }
  .play {
    width: 40px;
    height: 40px;
    border-radius: var(--r-md);
    background: color-mix(in srgb, var(--on-accent) 18%, transparent);
    display: flex;
    align-items: center;
    justify-content: center;
  }
  .picks {
    display: grid;
    grid-template-columns: repeat(auto-fill, minmax(280px, 1fr));
    gap: 2px 16px;
  }
  /* One row of cards: as many as fit at 150px or wider (see `cols`). */
  .row {
    display: grid;
    grid-template-columns: repeat(var(--cols), minmax(0, 1fr));
    column-gap: 20px;
  }
  .tiles {
    display: grid;
    grid-template-columns: repeat(auto-fill, minmax(150px, 1fr));
    gap: 20px;
  }
  .tile {
    display: flex;
    flex-direction: column;
    gap: 6px;
    text-align: left;
    min-width: 0;
  }
  .art {
    width: 100%;
    aspect-ratio: 1;
    border-radius: var(--r-md);
  }
  .tt {
    font-weight: 600;
  }
  .ts {
    font-size: 13px;
    margin-top: -4px;
  }
  .more {
    margin-top: 4px;
  }
  /* Phone: song lists swipe sideways in columns of four rows, each column a little narrower
     than the screen so the next one peeks in; long lists no longer bury the sections below. */
  @media (max-width: 640px) {
    .picks {
      grid-template-columns: none;
      grid-template-rows: repeat(4, auto);
      grid-auto-flow: column;
      grid-auto-columns: calc(100% - 48px);
      gap: 0 8px;
      overflow-x: auto;
      scroll-snap-type: x mandatory;
      scroll-padding-inline: 8px;
      scrollbar-width: none;
      margin: 0 -16px;
      padding: 0 8px;
    }
    /* Each row's own 8px padding puts its cover on the 16px gutter. */
    .picks > :global(*) {
      margin: 0;
      scroll-snap-align: start;
    }
  }
  /* Phone: card rows and Recently played scroll sideways, edge to edge past the 16px gutter,
     with the next card peeking in so it's clear there's more. */
  @media (max-width: 640px) {
    .row,
    .tiles {
      display: grid;
      grid-template-columns: none;
      grid-auto-flow: column;
      grid-auto-columns: 140px;
      gap: 16px;
      overflow-x: auto;
      scroll-snap-type: x mandatory;
      scroll-padding-inline: 16px;
      scrollbar-width: none;
      margin: 0 -16px;
      padding: 8px 16px;
    }
    .row > :global(*),
    .tile {
      scroll-snap-align: start;
    }
    .row {
      padding-inline: 24px;
      scroll-padding-inline: 24px;
    }
  }
  .empty {
    display: flex;
    flex-direction: column;
    gap: 12px;
    align-items: flex-start;
    max-width: 520px;
  }
  .empty p {
    margin: 0;
    font-size: 15px;
    line-height: 1.5;
  }
</style>
