<script lang="ts">
  import { img, type AlbumPage } from '../api'
  import { loadAlbum, openAddMenu, playList, shuffled, ui } from '../state.svelte'
  import Icon from '../components/Icon.svelte'
  import TrackRow from '../components/TrackRow.svelte'
  import ArtistLinks from '../components/ArtistLinks.svelte'
  import BackButton from '../components/BackButton.svelte'

  let album = $state<AlbumPage | null>(null)
  let failed = $state(false)

  function load() {
    const id = ui.albumId
    if (!id) return
    failed = false
    loadAlbum(id)
      .then((a) => {
        if (ui.albumId === id) album = a
      })
      .catch(() => {
        if (ui.albumId === id) failed = true
      })
  }
  load()

  const totalMin = $derived(Math.round((album?.tracks ?? []).reduce((s, t) => s + (t.duration ?? 0), 0) / 60))
  // Only show a song's artists when they differ from the album's (features, compilations).
  const albumArtists = $derived((album?.card.artists ?? []).map((a) => a.name).join(', '))
</script>

<div class="page scroll">
  <BackButton />
  {#if album}
    {@const a = album.card}
    <header>
      {#if album.cover}<img class="cover art" src={img(album.cover)} alt={`${a.title} cover`} />{:else}<div class="cover art"></div>{/if}
      <div class="info">
        <p class="kind">{a.kind}{a.year ? ` · ${a.year}` : ''}</p>
        <h1>{a.title}</h1>
        <p class="by"><ArtistLinks artists={a.artists} /></p>
        <p class="muted meta">
          {album.tracks.length} {album.tracks.length === 1 ? 'song' : 'songs'}{totalMin ? `, about ${totalMin} min` : ''}
        </p>
        <div class="actions">
          <button class="primary" disabled={!album.tracks.length} onclick={() => album && playList(album.tracks, 0)}>
            <Icon name="play" size={16} /><span class="trim">Play</span>
          </button>
          <button class="secondary" disabled={!album.tracks.length} onclick={() => album && playList(shuffled(album.tracks), 0)}>
            <Icon name="shuffle" size={16} /><span class="trim">Shuffle</span>
          </button>
          <button class="icon-btn" aria-label="Add all songs to a playlist" title="Add to playlist" disabled={!album.tracks.length}
            onclick={(e) => album && openAddMenu(album.tracks, e)}>
            <Icon name="plus" size={20} />
          </button>
        </div>
      </div>
    </header>

    <div class="list">
      {#each album.tracks as t, i (t.id)}
        <TrackRow track={t} number={i + 1} detail={t.artists.join(', ') === albumArtists ? '' : undefined}
          onplay={() => album && playList(album.tracks, i)} />
      {/each}
    </div>
  {:else if failed}
    <p class="muted retry">Couldn't load this album. Check your connection.</p>
    <button class="secondary fit" onclick={load}><span class="trim">Try again</span></button>
  {:else}
    <header aria-busy="true">
      <div class="cover art"></div>
      <div class="info"><div class="ghost t1"></div><div class="ghost t2"></div></div>
    </header>
  {/if}
</div>

<style>
  .page {
    height: 100%;
    padding: 0 32px 40px;
    display: flex;
    flex-direction: column;
    gap: 20px;
  }
  header {
    margin-top: 76px;
    display: flex;
    gap: 28px;
    align-items: flex-end;
    flex-wrap: wrap;
  }
  .art {
    width: 220px;
    height: 220px;
    border-radius: 14px;
    flex: none;
    box-shadow: 0 12px 40px rgb(0 0 0 / 0.3);
  }
  .info {
    display: flex;
    flex-direction: column;
    gap: 6px;
    min-width: 0;
    flex: 1 1 300px;
  }
  .info p {
    margin: 0;
  }
  .kind {
    font-size: 13px;
    font-weight: 700;
    color: var(--muted);
    text-transform: uppercase;
    letter-spacing: 0.06em;
  }
  h1 {
    font-size: clamp(30px, 4vw, 48px);
    font-weight: 800;
    letter-spacing: -0.03em;
    line-height: 1.05;
  }
  .by {
    font-size: 17px;
    font-weight: 600;
  }
  .meta {
    font-size: 14px;
  }
  .actions {
    display: flex;
    align-items: center;
    gap: 8px;
    flex-wrap: wrap;
    margin-top: 10px;
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
  .secondary {
    background: var(--raised);
  }
  .retry {
    margin-top: 76px;
  }
  .fit {
    align-self: flex-start;
  }
  button:disabled {
    opacity: 0.4;
    cursor: default;
  }
  .list {
    display: flex;
    flex-direction: column;
    max-width: 900px;
  }
  .ghost {
    background: var(--raised);
    border-radius: 10px;
  }
  .t1 {
    width: min(320px, 80%);
    height: 44px;
  }
  .t2 {
    width: 160px;
    height: 18px;
  }
</style>
