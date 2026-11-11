<script lang="ts">
  import { img, type ArtistPage } from '../api'
  import { loadArtist, playArtistRadio, playList, ui } from '../state.svelte'
  import Icon from '../components/Icon.svelte'
  import TrackRow from '../components/TrackRow.svelte'
  import AlbumCard from '../components/AlbumCard.svelte'
  import BackButton from '../components/BackButton.svelte'

  let artist = $state<ArtistPage | null>(null)
  let failed = $state(false)

  function load() {
    const id = ui.artistId
    if (!id) return
    failed = false
    loadArtist(id)
      .then((a) => {
        if (ui.artistId === id) artist = a
      })
      .catch(() => {
        if (ui.artistId === id) failed = true
      })
  }
  load()
</script>

<div class="page scroll">
  <BackButton />
  {#if artist}
    <header class="hero" class:plain={!artist.banner}>
      {#if artist.banner}<img class="banner" src={img(artist.banner)} alt="" />{/if}
      <div class="shade"></div>
      <div class="who">
        <h1>{artist.name}</h1>
        {#if artist.subscribers}<p class="subs">{artist.subscribers} subscribers</p>{/if}
      </div>
    </header>

    <div class="actions">
      <button class="primary" disabled={!artist.top.length} onclick={() => artist && playList(artist.top, 0)}>
        <Icon name="play" size={16} /><span class="trim">Play</span>
      </button>
      <button class="secondary" onclick={() => artist && playArtistRadio(artist.id, artist.name)}>
        <Icon name="radio" size={16} /><span class="trim">Radio</span>
      </button>
    </div>

    {#if artist.top.length}
      <section>
        <h2>Top songs</h2>
        <div class="list">
          {#each artist.top as t, i (t.id)}
            <TrackRow track={t} showDuration={false} detail={t.album ?? undefined} onplay={() => artist && playList(artist.top, i)} />
          {/each}
        </div>
      </section>
    {/if}

    {#if artist.albums.length}
      <section>
        <h2>Albums</h2>
        <div class="grid">
          {#each artist.albums as a (a.id)}<AlbumCard album={a} />{/each}
        </div>
      </section>
    {/if}

    {#if artist.singles.length}
      <section>
        <h2>Singles and EPs</h2>
        <div class="grid">
          {#each artist.singles as a (a.id)}<AlbumCard album={a} />{/each}
        </div>
      </section>
    {/if}
  {:else if failed}
    <p class="muted retry">Couldn't load this artist. Check your connection.</p>
    <button class="secondary fit" onclick={load}><span class="trim">Try again</span></button>
  {:else}
    <header class="hero plain loading" aria-busy="true">
      <div class="who"><div class="ghost name"></div><div class="ghost subs-ghost"></div></div>
    </header>
  {/if}
</div>

<style>
  .page {
    height: 100%;
    padding: 0 32px 40px;
    display: flex;
    flex-direction: column;
    gap: 28px;
  }
  .hero {
    position: relative;
    margin: 0 -32px -8px;
    height: clamp(240px, 30vw, 360px);
    display: flex;
    flex-direction: column;
    justify-content: flex-end;
    padding: 76px 32px 0;
    flex: none;
    overflow: hidden;
  }
  .hero.plain {
    height: auto;
    min-height: 180px;
  }
  .banner {
    position: absolute;
    inset: 0;
    width: 100%;
    height: 100%;
    object-fit: cover;
    object-position: center 25%;
  }
  .shade {
    position: absolute;
    inset: 0;
    background: linear-gradient(to bottom, rgb(0 0 0 / 0.25) 0%, transparent 35%, color-mix(in srgb, var(--bg) 70%, transparent) 72%, var(--bg) 100%);
  }
  .plain .shade {
    display: none;
  }
  .who {
    position: relative;
  }
  .retry {
    margin-top: 76px;
  }
  h1 {
    font-size: clamp(36px, 5.5vw, 64px);
    font-weight: 800;
    letter-spacing: -0.035em;
    line-height: 1;
  }
  .subs {
    margin: 8px 0 0;
    color: var(--muted);
    font-weight: 600;
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
  .fit {
    align-self: flex-start;
  }
  button:disabled {
    opacity: 0.4;
    cursor: default;
  }
  h2 {
    font-size: 18px;
    font-weight: 700;
    margin-bottom: 12px;
  }
  .list {
    display: flex;
    flex-direction: column;
  }
  .grid {
    display: grid;
    grid-template-columns: repeat(auto-fill, minmax(160px, 1fr));
    gap: 24px 20px;
  }
  .ghost {
    background: var(--raised);
    border-radius: var(--r-md);
  }
  .ghost.name {
    width: min(360px, 70%);
    height: 56px;
  }
  .subs-ghost {
    width: 140px;
    height: 18px;
    margin-top: 12px;
  }
</style>
