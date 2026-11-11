<script lang="ts">
  import { playList, ui } from '../state.svelte'
  import TrackRow from '../components/TrackRow.svelte'
</script>

<div class="page scroll">
  <h1>Library</h1>
  <p class="muted">Liked songs and playlists arrive with the next version. For now, here's what you've played.</p>
  <section>
    <h2>History</h2>
    {#if ui.recent.length}
      <div class="list">
        {#each ui.recent as t, i (t.id)}
          <TrackRow track={t} onplay={() => playList(ui.recent, i)} />
        {/each}
      </div>
    {:else}
      <p class="muted">Songs you play show up here.</p>
    {/if}
  </section>
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
  h2 {
    font-size: 18px;
    font-weight: 700;
    margin: 12px 0;
  }
  p {
    margin: 0;
  }
  .list {
    display: flex;
    flex-direction: column;
    max-width: 760px;
  }
</style>
