<script lang="ts">
  import { artistLine, img } from '../api'
  import { go, playRadio, ui, type View } from '../state.svelte'
  import Icon from './Icon.svelte'

  const items: { view: View; label: string; icon: string }[] = [
    { view: 'home', label: 'Home', icon: 'home' },
    { view: 'search', label: 'Search', icon: 'search' },
    { view: 'library', label: 'Library', icon: 'library' },
  ]
</script>

<nav class="side" aria-label="Main">
  <div class="logo">
    <span class="mark" aria-hidden="true">
      <svg width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2.6" stroke-linecap="round"><path d="M4 10v4M8 6v12M12 9v6M16 4v16M20 10v4" /></svg>
    </span>
    hum
  </div>
  {#each items as it}
    <button class="link" class:active={ui.view === it.view && !ui.nowPlayingOpen} onclick={() => go(it.view)}>
      <Icon name={it.icon} />{it.label}
    </button>
  {/each}

  {#if ui.recent.length}
    <h2 class="label">Recently played</h2>
    <div class="recent scroll">
      {#each ui.recent.slice(0, 8) as t (t.id)}
        <button class="mini" onclick={() => playRadio(t)} title={`Radio from ${t.title}`}>
          {#if t.thumb}<img class="cover" src={img(t.thumb)} alt="" loading="lazy" />{:else}<div class="cover"></div>{/if}
          <span class="text"><span class="ellipsis t">{t.title}</span><span class="ellipsis muted s">{artistLine(t)}</span></span>
        </button>
      {/each}
    </div>
  {/if}

  <button class="link settings" class:active={ui.view === 'settings' && !ui.nowPlayingOpen} onclick={() => go('settings')}>
    <Icon name="settings" />Settings
  </button>
</nav>

<style>
  .side {
    display: flex;
    flex-direction: column;
    gap: 2px;
    padding: 18px 12px;
    border-right: 1px solid var(--line);
    min-height: 0;
  }
  .logo {
    display: flex;
    align-items: center;
    gap: 10px;
    padding: 4px 12px 18px;
    font-weight: 800;
    font-size: 20px;
    letter-spacing: -0.02em;
  }
  .mark {
    width: 28px;
    height: 28px;
    border-radius: 8px;
    background: var(--accent, var(--accent-default));
    color: var(--on-accent);
    display: flex;
    align-items: center;
    justify-content: center;
    transition: background 0.4s;
  }
  .link {
    display: flex;
    align-items: center;
    gap: 12px;
    padding: 0 12px;
    min-height: 44px;
    border-radius: 10px;
    color: var(--muted);
    font-weight: 600;
    text-align: left;
  }
  .link:hover {
    color: var(--text);
  }
  .link.active {
    background: var(--raised);
    color: var(--text);
  }
  .label {
    font-size: 13px;
    font-weight: 700;
    color: var(--muted);
    margin: 18px 12px 6px;
    letter-spacing: 0;
  }
  .recent {
    display: flex;
    flex-direction: column;
    gap: 2px;
    min-height: 0;
    flex: 1;
  }
  .mini {
    display: flex;
    align-items: center;
    gap: 10px;
    padding: 6px 12px;
    border-radius: 10px;
    text-align: left;
    min-height: 44px;
  }
  .mini:hover {
    background: var(--raised);
  }
  .mini .cover {
    width: 32px;
    height: 32px;
    border-radius: 6px;
    flex: none;
  }
  .text {
    display: flex;
    flex-direction: column;
    min-width: 0;
  }
  .t {
    font-weight: 600;
    font-size: 13px;
  }
  .s {
    font-size: 12px;
  }
  .settings {
    margin-top: auto;
  }
</style>
