<script lang="ts">
  import { api, type HistoryEntry, type Track } from '../api'
  import { newPlaylistFromSidebar, openPlaylist, playList, player, shuffled, ui } from '../state.svelte'
  import Icon from '../components/Icon.svelte'
  import Mosaic from '../components/Mosaic.svelte'
  import TrackRow from '../components/TrackRow.svelte'

  let liked = $state<Track[]>([])
  let history = $state<HistoryEntry[]>([])
  let loaded = $state(false)

  $effect(() => {
    api.syncLikes().catch(() => {})
  })
  // Reload when the song changes or the account sync lands, so new likes and plays show up.
  $effect(() => {
    void player.current?.id
    void player.liked
    void ui.synced
    Promise.all([api.liked(), api.history(200)])
      .then(([l, h]) => {
        liked = l
        history = h
      })
      .catch(() => {})
      .finally(() => (loaded = true))
  })

  const historyTracks = $derived(history.map((h) => h.track))
  function when(ts: number): string {
    const d = new Date(ts * 1000)
    const today = new Date()
    const days = Math.floor((today.setHours(0, 0, 0, 0) - new Date(d).setHours(0, 0, 0, 0)) / 86400000)
    const time = d.toLocaleTimeString('en-GB', { hour: '2-digit', minute: '2-digit' })
    if (days === 0) return `Today ${time}`
    if (days === 1) return `Yesterday ${time}`
    return d.toLocaleDateString('en-GB', { day: 'numeric', month: 'short' })
  }
  const outcomeText = { completed: '', skipped_early: ', skipped', skipped_late: ', stopped early' }
</script>

<div class="page scroll">
  <h1>Library</h1>
  <div class="tabs" role="tablist">
    <!-- Phones have no sidebar, so their playlists live here. -->
    <button class="phone-only" role="tab" aria-selected={ui.libraryTab === 'playlists'} class:sel={ui.libraryTab === 'playlists'} onclick={() => (ui.libraryTab = 'playlists')}>
      <span class="trim">Playlists</span>
    </button>
    <button role="tab" aria-selected={ui.libraryTab === 'liked'} class:sel={ui.libraryTab === 'liked'} onclick={() => (ui.libraryTab = 'liked')}>
      <span class="trim">Liked songs{liked.length ? ` (${liked.length})` : ''}</span>
    </button>
    <button role="tab" aria-selected={ui.libraryTab === 'history'} class:sel={ui.libraryTab === 'history'} onclick={() => (ui.libraryTab = 'history')}>
      <span class="trim">History</span>
    </button>
  </div>

  {#if ui.libraryTab === 'playlists'}
    <div class="list">
      <button class="pl" onclick={newPlaylistFromSidebar}>
        <span class="new" aria-hidden="true"><Icon name="plus" size={22} /></span>
        <span class="t">New playlist</span>
      </button>
      <!-- Liked songs and History first, like pinned playlists. -->
      <button class="pl" onclick={() => (ui.libraryTab = 'liked')}>
        <span class="new liked" aria-hidden="true"><Icon name="heart" size={22} /></span>
        <span class="text"><span class="ellipsis t">Liked songs</span><span class="ellipsis muted s">{liked.length} {liked.length === 1 ? 'song' : 'songs'}</span></span>
      </button>
      <button class="pl" onclick={() => (ui.libraryTab = 'history')}>
        <span class="new" aria-hidden="true"><Icon name="clock" size={22} /></span>
        <span class="text"><span class="ellipsis t">History</span><span class="ellipsis muted s">Songs you've played</span></span>
      </button>
      {#each ui.playlists as p (p.id)}
        <button class="pl" onclick={() => openPlaylist(p.id)}>
          <Mosaic covers={p.covers} size={52} radius={8} />
          <span class="text"><span class="ellipsis t">{p.name}</span><span class="ellipsis muted s">{p.count} {p.count === 1 ? 'song' : 'songs'}</span></span>
        </button>
      {/each}
    </div>
  {:else if ui.libraryTab === 'liked'}
    {#if liked.length}
      <div class="actions">
        <button class="primary" onclick={() => playList(liked, 0)}><Icon name="play" size={16} /><span class="trim">Play</span></button>
        <button class="secondary" onclick={() => playList(shuffled(liked), 0)}><Icon name="shuffle" size={16} /><span class="trim">Shuffle</span></button>
      </div>
      <div class="list">
        {#each liked as t, i (t.id)}
          <TrackRow track={t} onplay={() => playList(liked, i)} />
        {/each}
      </div>
    {:else if loaded}
      <p class="muted">Tap the heart on a song you love and it lands here.</p>
    {/if}
  {:else if history.length}
    <div class="list">
      {#each history as h, i (h.played_at + h.track.id)}
        <TrackRow track={h.track} suffix={`, ${when(h.played_at)}${outcomeText[h.outcome]}`}
          onplay={() => playList(historyTracks, i)} />
      {/each}
    </div>
  {:else if loaded}
    <p class="muted">Songs you play show up here.</p>
  {/if}
</div>

<style>
  .page {
    height: 100%;
    padding: 28px 32px 40px;
    display: flex;
    flex-direction: column;
    gap: 16px;
  }
  h1 {
    font-size: 24px;
    font-weight: 700;
  }
  p {
    margin: 0;
  }
  .tabs {
    display: flex;
    gap: 6px;
  }
  .tabs button {
    height: 40px;
    padding: 0 18px;
    border-radius: 999px;
    font-weight: 600;
    background: var(--raised);
  }
  .tabs button.sel {
    background: var(--text);
    color: var(--bg);
  }
  .phone-only {
    display: none;
  }
  .actions {
    display: flex;
    gap: 8px;
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
    color: var(--muted);
    padding: 0 14px;
  }
  .secondary:hover {
    color: var(--text);
    background: var(--raised);
  }
  .pl {
    display: flex;
    align-items: center;
    gap: 14px;
    min-height: 64px;
    padding: 6px 8px;
    margin: 0 -8px;
    border-radius: var(--r-md);
    text-align: left;
  }
  .pl:hover {
    background: var(--raised);
  }
  .new {
    width: 52px;
    height: 52px;
    flex: none;
    border-radius: 8px;
    background: var(--raised);
    display: flex;
    align-items: center;
    justify-content: center;
  }
  .new.liked {
    background: var(--accent, var(--accent-default));
    color: var(--on-accent);
  }
  .text {
    display: flex;
    flex-direction: column;
    min-width: 0;
  }
  .t {
    font-weight: 600;
  }
  .s {
    font-size: 13px;
  }
  @media (max-width: 640px) {
    .phone-only {
      display: block;
    }
    /* Three pills don't fit 360px with desktop padding: tighten, and scroll if they still don't. */
    .tabs {
      overflow-x: auto;
      scrollbar-width: none;
      flex: none;
    }
    .tabs button {
      flex: none;
      padding: 0 14px;
    }
  }
  .list {
    display: flex;
    flex-direction: column;
    max-width: 760px;
  }
</style>
