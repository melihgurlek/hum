<script lang="ts">
  import { go, openLibrary, ui, type View } from '../state.svelte'
  import Icon from './Icon.svelte'

  // On a phone this replaces the sidebar; playlists live in Library there.
  const tabs: { view: View; label: string; icon: string; also?: View[] }[] = [
    { view: 'home', label: 'Home', icon: 'home', also: ['artist', 'album'] },
    { view: 'search', label: 'Search', icon: 'search' },
    { view: 'library', label: 'Library', icon: 'library', also: ['playlist'] },
    { view: 'settings', label: 'Settings', icon: 'settings' },
  ]
  const active = (t: (typeof tabs)[number]) => !ui.nowPlayingOpen && (ui.view === t.view || !!t.also?.includes(ui.view))
</script>

<nav class="tabs" aria-label="Main">
  {#each tabs as t}
    <button class="tab" class:active={active(t)} aria-current={active(t) ? 'page' : undefined} onclick={() => {
      // Library opens on the list of your collections, as phone music apps do.
      if (t.view === 'library') openLibrary()
      else go(t.view)
    }}>
      <Icon name={t.icon} size={22} />
      <span>{t.label}</span>
    </button>
  {/each}
</nav>

<style>
  .tabs {
    display: grid;
    grid-template-columns: repeat(4, 1fr);
    padding: 4px 8px calc(4px + env(safe-area-inset-bottom));
    background: var(--bar);
  }
  .tab {
    display: flex;
    flex-direction: column;
    align-items: center;
    justify-content: center;
    gap: 3px;
    min-height: 56px;
    color: var(--muted);
    font-size: 11px;
    font-weight: 600;
    border-radius: var(--r-md);
    -webkit-tap-highlight-color: transparent;
  }
  .tab.active {
    color: var(--text);
  }
  .tab.active span {
    font-weight: 700;
  }
</style>
