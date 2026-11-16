<script lang="ts">
  import { api, artistLine, type HomeCard, type QuickPicks, type Track, img } from '../api'
  import { untrack } from 'svelte'
  import { capSet, go, homeFeed, layout, openArtist, savedPicks, playList, playRadio, playYtPlaylist, prefetchOnHover, quickPicksCache as cache, ui } from '../state.svelte'
  import AlbumCard from '../components/AlbumCard.svelte'
  import Icon from '../components/Icon.svelte'
  import TrackRow from '../components/TrackRow.svelte'

  // Quick picks: up to four seeds' radios (your latest plays and favorites, or your likes on a
  // fresh install) fused and ranked by your taste. Cached for the session, keyed by the latest
  // play, so returning to Home is instant. The key is read once per visit: playing something
  // from Home shouldn't swap the list under your finger. (It waits for the library's first
  // answer, so a cold start still keys on the latest play.)
  let key = $state(untrack(() => ui.recent[0]?.id ?? ''))
  // Recently played, likewise as it was when you came to Home.
  let recentShown = $state(untrack(() => ui.recent.slice(0, 12)))
  $effect(() => {
    const first = ui.recent[0]?.id ?? ''
    if (!key && first) key = first
    if (!recentShown.length && ui.recent.length) recentShown = ui.recent.slice(0, 12)
  })
  // On a cold start the list takes a few radio lookups, so the last one is shown meanwhile.
  // The fresh list replaces it only if you haven't started using it, so nothing moves under
  // your finger; otherwise it's saved for next time.
  let qp = $state<QuickPicks | null>(null)
  let loading = $state(false)
  let touched = false
  $effect(() => {
    const k = key
    if (cache.has(k)) {
      qp = cache.get(k)!
      return
    }
    const saved = untrack(() => qp) ?? savedPicks.get()
    if (saved) qp = saved
    loading = !saved
    api
      .quickPicks()
      .then((r) => {
        capSet(cache, k, r, 10)
        savedPicks.set(r)
        if (key === k && !(saved && touched)) qp = r
      })
      .catch(() => {})
      .finally(() => (loading = false))
  })
  // What Quick picks are built from, for the loading line (from the same snapshot).
  const seed = $derived(untrack(() => ui.recent.find((t) => t.id === key)) ?? qp?.seeds[0] ?? null)
  const picks = $derived(qp?.picks ?? [])
  // Desktop shows the first 12 (about four rows) so the sections below stay in sight; phones
  // swipe through all of them four rows at a time.
  let allPicks = $state(false)
  const shownPicks = $derived(layout.phone || allPicks ? picks : picks.slice(0, 12))

  let forgotten = $state<{ track: Track; last_played: number }[]>([])
  $effect(() => {
    api.forgotten(8).then((f) => (forgotten = f)).catch(() => {})
  })
  // The artists you play most, each shown with the cover of their best-liked song.
  let artists = $state<Track[]>([])
  $effect(() => {
    api.topArtists(12).then((a) => (artists = a)).catch(() => {})
  })
  const monthYear = (ts: number) =>
    new Date(ts * 1000).toLocaleDateString('en-GB', { month: 'long', year: 'numeric' })

  // Below hum's own sections, YouTube Music's home page (the account's when signed in): albums
  // and mixes for you, new releases, moods. On a first launch it is all there is. Once you've
  // played here, its song shelves (its Quick picks) repeat ours, From your library is the
  // Library tab, and community playlists are strangers' picks; those are left out.
  const fresh = $derived(ui.recent.length === 0)
  // YouTube changes it through the day (morning, workout, evening shelves) and the phone app can
  // live for days, so a visit fetches it again once it's an hour old; the old one shows meanwhile.
  $effect(() => {
    if (homeFeed.shelves && Date.now() - homeFeed.at < 3600_000) return
    api.homeFeed().then((s) => Object.assign(homeFeed, { shelves: s, at: Date.now() })).catch(() => {})
  })
  const shelves = $derived(
    (homeFeed.shelves ?? []).filter(
      (s) => fresh || (!s.songs.length && s.cards.length && !/community|from your library/i.test(s.title)),
    ),
  )
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
    <section onpointerdown={() => (touched = true)}>
      <div class="head">
        <h2>{fresh ? 'From your liked songs' : 'Quick picks'}</h2>
      </div>
      {#if loading && !picks.length}
        <p class="muted">Finding songs like {seed.title}…</p>
      {:else}
        <div class="picks">
          {#each shownPicks as t, i (t.id)}
            <TrackRow track={t} showDuration={false} onplay={() => playList(picks, i)} />
          {/each}
        </div>
        {#if !layout.phone && picks.length > 12}
          <button class="link more" onclick={() => (allPicks = !allPicks)}>{allPicks ? 'Show fewer' : `Show all ${picks.length}`}</button>
        {/if}
      {/if}
    </section>

    {#if recentShown.length}
      <section>
        <h2>Recently played</h2>
        <div class="tiles">
          {#each recentShown as t (t.id)}
            <button class="tile" use:prefetchOnHover={t.id} onclick={() => playRadio(t)} title={`Radio from ${t.title}`}>
              {#if t.cover}<img class="cover art" src={img(t.cover)} alt="" loading="lazy" />{:else}<div class="cover art"></div>{/if}
              <span class="ellipsis tt">{t.title}</span>
              <span class="ellipsis muted ts">{artistLine(t)}</span>
            </button>
          {/each}
        </div>
      </section>
    {/if}

    {#if artists.length > 2}
      <section>
        <h2>Your artists</h2>
        <div class="row artists" style:--cols={cols} bind:clientWidth={rowWidth}>
          {#each artists.slice(0, shown) as t (t.artist_id)}
            <button class="artist" onclick={() => openArtist(t.artist_id)}>
              {#if t.cover}<img class="face" src={img(t.cover)} alt="" loading="lazy" />{:else}<div class="face"></div>{/if}
              <span class="ellipsis tt">{t.artists[0]}</span>
            </button>
          {/each}
        </div>
      </section>
    {/if}

    {#if forgotten.length}
      <section>
        <div class="head">
          <h2>Haven't played these in a while</h2>
        </div>
        <div class="picks">
          {#each forgotten as f, i (f.track.id)}
            <TrackRow track={f.track} showDuration={false} suffix={`, last played ${monthYear(f.last_played)}`}
              onplay={() => playList(forgotten.map((x) => x.track), i)} />
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

  {#if shelves.length}
    {#each shelves as shelf (shelf.title)}
      <section>
        <div class="head">
          <h2>{shelf.title}</h2>
        </div>
        {#if shelf.songs.length}
          <div class="picks">
            {#each shelf.songs.slice(0, 8) as t, i (t.id)}
              <TrackRow track={t} showDuration={false} onplay={() => playList(shelf.songs, i)} />
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
  .artist {
    display: flex;
    flex-direction: column;
    align-items: center;
    gap: 8px;
    min-width: 0;
    text-align: center;
  }
  .artist .tt {
    max-width: 100%;
  }
  .face {
    width: 100%;
    aspect-ratio: 1;
    border-radius: 50%;
    object-fit: cover;
    background: var(--surface);
  }
  /* Phone: song lists swipe sideways in columns of four rows, each column a little narrower
     than the screen so the next one peeks in; long lists no longer bury the sections below. */
  @media (max-width: 640px) {
    .picks {
      grid-template-columns: none;
      grid-template-rows: repeat(4, auto);
      grid-auto-flow: column;
      grid-auto-columns: calc(100% - 40px);
      gap: 0 24px;
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
    /* Circles have no card padding of their own: on the gutter, like Recently played. */
    .row.artists {
      padding-inline: 16px;
      scroll-padding-inline: 16px;
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
