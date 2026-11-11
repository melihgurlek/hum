<script lang="ts">
  import { onMount } from 'svelte'
  import { back, go, startListening, ui } from './lib/state.svelte'
  import Sidebar from './lib/components/Sidebar.svelte'
  import PlayerBar from './lib/components/PlayerBar.svelte'
  import Home from './lib/views/Home.svelte'
  import Search from './lib/views/Search.svelte'
  import Library from './lib/views/Library.svelte'
  import Settings from './lib/views/Settings.svelte'
  import NowPlaying from './lib/views/NowPlaying.svelte'
  import PlaylistView from './lib/views/PlaylistView.svelte'
  import ArtistView from './lib/views/ArtistView.svelte'
  import AlbumView from './lib/views/AlbumView.svelte'
  import AddMenu from './lib/components/AddMenu.svelte'

  onMount(() => {
    startListening()
  })

  function keys(e: KeyboardEvent) {
    if ((e.ctrlKey || e.metaKey) && e.key.toLowerCase() === 'k') {
      e.preventDefault()
      go('search')
    }
    if (e.altKey && e.key === 'ArrowLeft') {
      e.preventDefault()
      back()
    }
  }
  // Mouse "back" side button.
  function mouse(e: MouseEvent) {
    if (e.button === 3) {
      e.preventDefault()
      back()
    }
  }
</script>

<svelte:window onkeydown={keys} onmouseup={mouse} />

<div class="app">
  <Sidebar />
  <main>
    {#if ui.view === 'home'}<Home />
    {:else if ui.view === 'search'}<Search />
    {:else if ui.view === 'library'}<Library />
    {:else if ui.view === 'playlist'}<PlaylistView />
    {:else if ui.view === 'artist'}{#key ui.artistId}<ArtistView />{/key}
    {:else if ui.view === 'album'}{#key ui.albumId}<AlbumView />{/key}
    {:else}<Settings />{/if}
    {#if ui.nowPlayingOpen}<NowPlaying />{/if}
  </main>
  <div class="player"><PlayerBar /></div>
  <AddMenu />
  {#if ui.toast}<div class="toast" role="status">{ui.toast}</div>{/if}
</div>

<style>
  .app {
    height: 100%;
    display: grid;
    grid-template-columns: 240px 1fr;
    grid-template-rows: 1fr auto;
  }
  main {
    position: relative;
    min-width: 0;
    min-height: 0;
  }
  .player {
    grid-column: 1 / -1;
  }
  .toast {
    position: fixed;
    left: 50%;
    bottom: 100px;
    transform: translateX(-50%);
    background: var(--text);
    color: var(--bg);
    padding: 12px 18px;
    border-radius: var(--r-md);
    font-weight: 600;
    max-width: 90vw;
    z-index: 20;
  }
  @media (max-width: 860px) {
    .app {
      grid-template-columns: 72px 1fr;
    }
  }
</style>
