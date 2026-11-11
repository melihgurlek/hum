<script lang="ts">
  import { api, type HistoryEntry, type Track } from '../api'
  import { playList, player, ui } from '../state.svelte'
  import TrackRow from '../components/TrackRow.svelte'

  let liked = $state<Track[]>([])
  let history = $state<HistoryEntry[]>([])
  let loaded = $state(false)

  // Reload when the song changes, so a new like or play shows up without leaving the page.
  $effect(() => {
    void player.current?.id
    void player.liked
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
    <button role="tab" aria-selected={ui.libraryTab === 'liked'} class:sel={ui.libraryTab === 'liked'} onclick={() => (ui.libraryTab = 'liked')}>
      <span class="trim">Liked songs{liked.length ? ` (${liked.length})` : ''}</span>
    </button>
    <button role="tab" aria-selected={ui.libraryTab === 'history'} class:sel={ui.libraryTab === 'history'} onclick={() => (ui.libraryTab = 'history')}>
      <span class="trim">History</span>
    </button>
  </div>

  {#if ui.libraryTab === 'liked'}
    {#if liked.length}
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
        <TrackRow track={h.track} detail={`${h.track.artists.join(', ')}, ${when(h.played_at)}${outcomeText[h.outcome]}`}
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
    font-size: 30px;
    font-weight: 800;
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
  .list {
    display: flex;
    flex-direction: column;
    max-width: 760px;
  }
</style>
