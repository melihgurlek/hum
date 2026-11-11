<script lang="ts">
  import { addToNewPlaylist, addToPlaylist, ui } from '../state.svelte'
  import Mosaic from './Mosaic.svelte'

  let name = $state('')
  let menu: HTMLDivElement | undefined = $state()

  // Keep the menu on screen.
  const pos = $derived.by(() => {
    const m = ui.addMenu
    if (!m) return { left: 0, top: 0 }
    const w = 280
    const h = Math.min(420, 120 + ui.playlists.length * 52)
    return {
      left: Math.max(8, Math.min(m.x - w, window.innerWidth - w - 8)),
      top: Math.max(8, Math.min(m.y, window.innerHeight - h - 8)),
    }
  })

  $effect(() => {
    if (ui.addMenu) {
      name = ''
      menu?.querySelector<HTMLInputElement>('input')?.focus()
    }
  })

  function close() {
    ui.addMenu = null
  }
  function create() {
    if (ui.addMenu) addToNewPlaylist(name || 'New playlist', ui.addMenu.tracks)
  }
  const count = $derived(ui.addMenu?.tracks.length ?? 0)
</script>

<svelte:window onkeydown={(e) => e.key === 'Escape' && close()} />

{#if ui.addMenu}
  <button class="backdrop" aria-label="Close menu" onclick={close} oncontextmenu={(e) => (e.preventDefault(), close())}></button>
  <div class="menu" bind:this={menu} role="dialog" aria-label="Add to playlist" style="left:{pos.left}px;top:{pos.top}px">
    <div class="title">Add {count === 1 ? 'song' : `${count} songs`} to</div>
    <form class="new" onsubmit={(e) => (e.preventDefault(), create())}>
      <label for="np-name" class="sr">New playlist name</label>
      <input id="np-name" bind:value={name} placeholder="New playlist" autocomplete="off" />
      <button type="submit" class="create"><span class="trim">Create</span></button>
    </form>
    <div class="list scroll">
      {#each ui.playlists as p (p.id)}
        <button class="item" onclick={() => ui.addMenu && addToPlaylist(p.id, ui.addMenu.tracks)}>
          <Mosaic covers={p.covers} size={36} radius={5} />
          <span class="text"><span class="ellipsis">{p.name}</span><span class="muted n">{p.count} {p.count === 1 ? 'song' : 'songs'}</span></span>
        </button>
      {:else}
        <p class="muted empty">No playlists yet. Name one above.</p>
      {/each}
    </div>
  </div>
{/if}

<style>
  .backdrop {
    position: fixed;
    inset: 0;
    z-index: 30;
    cursor: default;
  }
  .menu {
    position: fixed;
    z-index: 31;
    width: 280px;
    max-height: 420px;
    display: flex;
    flex-direction: column;
    gap: 8px;
    padding: 12px;
    border-radius: var(--r-lg);
    background: var(--surface);
    border: 1px solid var(--line);
    box-shadow: 0 16px 40px rgba(0, 0, 0, 0.35);
  }
  .title {
    font-size: 13px;
    font-weight: 700;
    color: var(--muted);
    padding: 0 4px;
  }
  .sr {
    position: absolute;
    left: -9999px;
  }
  .new {
    display: flex;
    gap: 6px;
  }
  .new input {
    flex: 1;
    min-width: 0;
    height: 40px;
    padding: 0 12px;
    border-radius: var(--r-md);
    border: 1px solid var(--line);
    background: var(--bg);
    outline: none;
    user-select: text;
  }
  .new input:focus {
    border-color: var(--accent, var(--accent-default));
  }
  .create {
    height: 40px;
    padding: 0 14px;
    border-radius: var(--r-md);
    background: var(--accent, var(--accent-default));
    color: var(--on-accent);
    font-weight: 700;
  }
  .list {
    display: flex;
    flex-direction: column;
    min-height: 0;
  }
  .item {
    display: flex;
    align-items: center;
    gap: 10px;
    padding: 6px;
    border-radius: var(--r-md);
    text-align: left;
    min-height: 48px;
  }
  .item:hover {
    background: var(--raised);
  }
  .text {
    display: flex;
    flex-direction: column;
    min-width: 0;
    font-weight: 600;
  }
  .n {
    font-size: 12px;
    font-weight: 500;
  }
  .empty {
    font-size: 13px;
    padding: 4px;
    margin: 0;
  }
</style>
