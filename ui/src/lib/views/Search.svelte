<script lang="ts">
  import { api, artistLinks, img, type SearchResults, type Track } from '../api'
  import {
    forgetPick, openAlbum, openArtist, playAlbum, playArtistRadio, playArtistTop, playRadio,
    capSet, recentKey, rememberPick, searchCache, searches, ui, hoverPrefetch, hoverCancel, type Recent,
  } from '../state.svelte'
  import ArtistLinks from '../components/ArtistLinks.svelte'
  import Icon from '../components/Icon.svelte'
  import TrackRow from '../components/TrackRow.svelte'

  let results = $state<SearchResults>({ top: null, songs: [] })
  let searching = $state(false)
  let error = $state<string | null>(null)
  let lastQuery = ''
  let timer: ReturnType<typeof setTimeout> | undefined
  let input: HTMLInputElement


  async function run(q: string) {
    q = q.trim()
    if (!q || q === lastQuery) return
    lastQuery = q
    error = null
    if (searchCache.has(q)) {
      results = searchCache.get(q)!
      return
    }
    searching = true
    try {
      const r = await api.search(q)
      capSet(searchCache, q, r, 30)
      if (q === lastQuery) results = r
    } catch (e) {
      if (q === lastQuery) error = 'Search failed. Check your connection and try again.'
    } finally {
      if (q === lastQuery) searching = false
    }
  }

  function onInput() {
    clearTimeout(timer)
    timer = setTimeout(() => run(ui.query), 300)
  }

  $effect(() => {
    input?.focus()
    if (ui.query) run(ui.query)
  })

  function clear() {
    ui.query = ''
    results = { top: null, songs: [] }
    lastQuery = ''
    input.focus()
  }

  // YouTube Music's top result when it's an artist or album; otherwise the first song.
  const top = $derived(results.top)
  const topSong = $derived(top ? null : (results.songs[0] ?? null))
  const listed = $derived(top ? results.songs : results.songs.slice(1))

  // A song from search plays and then keeps going with songs like it. The other results only
  // share a word with the query, so they'd make a nonsense queue ("beta" → an unrelated song).
  function playSong(t: Track) {
    rememberPick({ kind: 'song', track: t })
    playRadio(t)
  }

  function openRecent(r: Recent) {
    rememberPick(r)
    if (r.kind === 'song') playRadio(r.track)
    else if (r.kind === 'artist') openArtist(r.id)
    else openAlbum(r.id)
  }
  const recentTitle = (r: Recent) => (r.kind === 'song' ? r.track.title : r.kind === 'artist' ? r.name : r.title)
  const recentSub = (r: Recent) =>
    r.kind === 'song' ? `Song · ${r.track.artists.join(', ')}` : r.kind === 'artist' ? 'Artist' : `Album${r.detail ? ' · ' + r.detail : ''}`
  const recentThumb = (r: Recent) => (r.kind === 'song' ? r.track.thumb : r.thumb)
</script>

<div class="page scroll">
  <div class="field">
    <Icon name="search" />
    <label for="q" class="sr">Search songs and artists</label>
    <input
      id="q"
      bind:this={input}
      bind:value={ui.query}
      oninput={onInput}
      onkeydown={(e) => e.key === 'Enter' && (clearTimeout(timer), run(ui.query))}
      placeholder="Songs, artists, albums"
      autocomplete="off"
      spellcheck="false"
    />
    {#if ui.query}
      <button class="clear" aria-label="Clear search" onclick={clear}>
        <Icon name="x" size={14} />
      </button>
    {/if}
  </div>

  {#if error}
    <p class="muted">{error}</p>
  {:else if !ui.query}
    {#if searches.list.length}
      <section class="recents">
        <div class="rhead">
          <h2>Recent searches</h2>
          <button class="link" onclick={() => forgetPick(null)}>Clear all</button>
        </div>
        <div class="rlist">
          {#each searches.list as r (recentKey(r))}
            <div class="rrow">
              <button class="ropen" onclick={() => openRecent(r)}>
                {#if recentThumb(r)}
                  <img class="cover rart" class:round={r.kind === 'artist'} src={img(recentThumb(r))} alt="" loading="lazy" width="48" height="48" />
                {:else}
                  <div class="cover rart" class:round={r.kind === 'artist'}></div>
                {/if}
                <span class="rtext"><span class="ellipsis rt">{recentTitle(r)}</span><span class="ellipsis rs">{recentSub(r)}</span></span>
              </button>
              <button class="x" aria-label={`Remove ${recentTitle(r)} from recent searches`} title="Remove" onclick={() => forgetPick(recentKey(r))}>
                <Icon name="x" size={14} />
              </button>
            </div>
          {/each}
        </div>
      </section>
    {/if}
  {:else if searching && !results.songs.length && !top}
    <p class="muted">Searching…</p>
  {:else if top || topSong}
    <div class="grid">
      <section class="topcol">
        <h2>Top result</h2>
        {#if top?.kind === 'artist'}
          {@const a = top}
          <div class="top">
            {#if a.thumb}<img class="cover topart round" src={img(a.thumb)} alt="" />{:else}<div class="cover topart round"></div>{/if}
            <div class="titles">
              <button class="toptitle open" onclick={() => (rememberPick({ kind: 'artist', id: a.id, name: a.name, thumb: a.thumb }), openArtist(a.id))}>{a.name}</button>
              <div class="muted meta">Artist{#if a.detail}<span class="dot" aria-hidden="true">·</span>{a.detail}{/if}</div>
            </div>
            <div class="actions">
              <button class="primary" onclick={() => (rememberPick({ kind: 'artist', id: a.id, name: a.name, thumb: a.thumb }), playArtistTop(a.id))}><Icon name="play" size={16} /><span class="trim">Play</span></button>
              <button class="secondary" onclick={() => (rememberPick({ kind: 'artist', id: a.id, name: a.name, thumb: a.thumb }), playArtistRadio(a.id, a.name))}><Icon name="radio" size={16} /><span class="trim">Radio</span></button>
            </div>
          </div>
        {:else if top?.kind === 'album'}
          {@const al = top}
          <div class="top">
            {#if al.thumb}<img class="cover topart" src={img(al.thumb)} alt="" />{:else}<div class="cover topart"></div>{/if}
            <div class="titles">
              <button class="toptitle open" onclick={() => (rememberPick(al), openAlbum(al.id))}>{al.title}</button>
              <div class="muted meta">Album{#if al.detail}<span class="dot" aria-hidden="true">·</span>{al.detail}{/if}</div>
            </div>
            <div class="actions">
              <button class="primary" onclick={() => (rememberPick(al), playAlbum(al.id))}><Icon name="play" size={16} /><span class="trim">Play</span></button>
              <button class="secondary" onclick={() => (rememberPick(al), playAlbum(al.id, true))}><Icon name="shuffle" size={16} /><span class="trim">Shuffle</span></button>
            </div>
          </div>
        {:else if topSong}
          {@const t = topSong}
          <div class="top" role="group" aria-label={t.title} onmouseenter={() => hoverPrefetch(t.id)} onmouseleave={hoverCancel}>
            {#if t.cover}<img class="cover topart" src={img(t.cover)} alt="" />{:else}<div class="cover topart"></div>{/if}
            <div class="titles">
              <div class="toptitle">{t.title}</div>
              <div class="muted meta">
                <ArtistLinks artists={artistLinks(t)} />{#if t.album}<span class="dot" aria-hidden="true">·</span>{#if t.album_id}<button class="inline-link" onclick={() => openAlbum(t.album_id)}>{t.album}</button>{:else}{t.album}{/if}{/if}
              </div>
            </div>
            <div class="actions">
              <button class="primary" onclick={() => playSong(t)}><Icon name="play" size={16} /><span class="trim">Play</span></button>
            </div>
          </div>
        {/if}
      </section>
      <section class="songs">
        <h2>Songs</h2>
        <div class="list">
          {#each listed as t (t.id)}
            <TrackRow track={t} onplay={() => playSong(t)} />
          {/each}
        </div>
      </section>
    </div>
  {:else}
    <p class="muted">Nothing found for “{ui.query}”. Try fewer words or check the spelling.</p>
  {/if}
</div>

<style>
  .page {
    height: 100%;
    padding: 24px 32px 40px;
    display: flex;
    flex-direction: column;
    gap: 24px;
  }
  .sr {
    position: absolute;
    left: -9999px;
  }
  .field {
    display: flex;
    align-items: center;
    gap: 10px;
    height: 48px;
    padding: 0 18px;
    border-radius: var(--r-lg);
    background: var(--surface);
    /* Quiet until focused: a permanent accent border reads as a stuck focus ring. */
    border: 2px solid var(--line);
    color: var(--muted);
    max-width: 720px;
    flex: none;
  }
  .field:focus-within {
    border-color: var(--accent, var(--accent-default));
  }
  .field input {
    flex: 1;
    border: 0;
    outline: none;
    background: transparent;
    font-size: 16px;
    color: var(--text);
    user-select: text;
    /* Plus Jakarta Sans has a tall ascent, so centered text sits ~2px low next to the icon.
       Uneven padding lifts it by 1.5px, centering it between its cap height and x-height. */
    padding: 0 0 3px;
  }
  .clear {
    width: 32px;
    height: 32px;
    border-radius: 50%;
    background: var(--raised);
    display: flex;
    align-items: center;
    justify-content: center;
  }
  .grid {
    display: flex;
    flex-wrap: wrap;
    gap: 32px;
  }
  h2 {
    font-size: 18px;
    font-weight: 700;
    margin-bottom: 12px;
  }
  .topcol {
    flex: 1 1 320px;
    max-width: 420px;
  }
  .top {
    display: flex;
    flex-direction: column;
    gap: 16px;
    padding: 24px;
    border-radius: var(--r-lg);
    background: var(--surface);
  }
  .topart {
    width: 140px;
    height: 140px;
    border-radius: var(--r-md);
  }
  .topart.round {
    border-radius: 50%;
  }
  .titles {
    display: flex;
    flex-direction: column;
    gap: 6px;
  }
  .toptitle {
    font-size: 24px;
    font-weight: 700;
    line-height: 1.15;
  }
  .toptitle.open {
    padding: 0;
    text-align: left;
    color: inherit;
  }
  .toptitle.open:hover,
  .toptitle.open:focus-visible {
    text-decoration: underline;
  }
  .meta {
    font-size: 15px;
  }
  .dot {
    margin: 0 6px;
  }
  .actions {
    display: flex;
    gap: 8px;
    flex-wrap: wrap;
  }
  .primary,
  .secondary {
    display: inline-flex;
    align-items: center;
    gap: 8px;
    height: 44px;
    padding: 0 20px;
    border-radius: 999px;
    font-weight: 700;
  }
  .primary {
    background: var(--accent, var(--accent-default));
    color: var(--on-accent);
  }
  /* One filled button per screen (Play); the rest stay quiet until hovered. */
  .secondary {
    background: transparent;
    color: var(--muted);
    padding: 0 14px;
  }
  .secondary:hover:not(:disabled) {
    color: var(--text);
    background: var(--raised);
  }
  .rhead {
    display: flex;
    align-items: center;
    justify-content: space-between;
    max-width: 720px;
  }
  .rhead h2 {
    margin: 0;
  }
  .link {
    color: var(--muted);
    font-weight: 600;
    min-height: 44px;
  }
  .link:hover {
    color: var(--text);
  }
  .rlist {
    display: flex;
    flex-direction: column;
    max-width: 720px;
  }
  .rrow {
    display: flex;
    align-items: center;
    border-radius: var(--r-md);
    padding-right: 8px;
  }
  .rrow:hover,
  .rrow:focus-within {
    background: var(--raised);
  }
  .ropen {
    flex: 1;
    min-width: 0;
    display: flex;
    align-items: center;
    gap: 12px;
    padding: 6px 8px;
    text-align: left;
    border-radius: var(--r-md);
  }
  .rart {
    width: 48px;
    height: 48px;
    border-radius: var(--r-sm);
    flex: none;
  }
  .rart.round {
    border-radius: 50%;
  }
  .rtext {
    display: flex;
    flex-direction: column;
    min-width: 0;
  }
  .rt {
    font-weight: 600;
  }
  .rs {
    color: var(--muted);
    font-size: 13px;
  }
  .x {
    width: 36px;
    height: 36px;
    border-radius: 50%;
    display: flex;
    align-items: center;
    justify-content: center;
    color: var(--muted);
    opacity: 0;
  }
  .rrow:hover .x,
  .x:focus-visible {
    opacity: 1;
  }
  .x:hover {
    color: var(--text);
    background: var(--raised-2);
  }
  .inline-link {
    display: inline;
    padding: 0;
    font: inherit;
    color: inherit;
  }
  .inline-link:hover,
  .inline-link:focus-visible {
    text-decoration: underline;
  }
  .songs {
    flex: 999 1 420px;
    min-width: 0;
  }
  .list {
    display: flex;
    flex-direction: column;
  }
</style>
