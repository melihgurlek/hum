<script lang="ts">
  import { api } from '../api'
  import { addToNewPlaylist, addToPlaylist, blockArtist, layout, refreshPlaylists, showToast, ui } from '../state.svelte'
  import Icon from './Icon.svelte'
  import Mosaic from './Mosaic.svelte'

  let name = $state('')
  let menu: HTMLDivElement | undefined = $state()

  // Keep the menu on screen.
  const pos = $derived.by(() => {
    const m = ui.addMenu
    if (!m) return { left: 0, top: 0 }
    const w = 280
    const h = Math.min(476, 176 + ui.playlists.length * 52)
    return {
      left: Math.max(8, Math.min(m.x - w, window.innerWidth - w - 8)),
      top: Math.max(8, Math.min(m.y, window.innerHeight - h - 8)),
    }
  })

  $effect(() => {
    if (ui.addMenu) {
      name = ''
      // Not on a phone: focusing would throw the keyboard over the list.
      if (!layout.phone) menu?.querySelector<HTMLInputElement>('input')?.focus()
    }
  })

  // For one song: the playlists it's already in (and where), checked; tapping one takes it out.
  let holding = $state<Map<number, number>>(new Map())
  $effect(() => {
    holding = new Map()
    const t = ui.addMenu?.tracks.length === 1 ? ui.addMenu.tracks[0] : null
    if (!t) return
    api.playlistsWith(t.id).then((r) => (holding = new Map(r))).catch(() => {})
  })
  async function pick(id: number) {
    const m = ui.addMenu
    if (!m) return
    const at = holding.get(id)
    if (at === undefined) return addToPlaylist(id, m.tracks)
    ui.addMenu = null
    const name = ui.playlists.find((p) => p.id === id)?.name ?? 'playlist'
    try {
      await api.removeFromPlaylist(id, at)
      showToast(`Removed from ${name}`)
      refreshPlaylists()
    } catch (e) {
      showToast(String(e))
    }
  }

  function close() {
    ui.addMenu = null
  }
  function create() {
    if (ui.addMenu) addToNewPlaylist(name || 'New playlist', ui.addMenu.tracks)
  }
  const count = $derived(ui.addMenu?.tracks.length ?? 0)
  // One song: offer to keep its artist out of radio.
  const single = $derived(ui.addMenu?.tracks.length === 1 ? ui.addMenu.tracks[0] : null)
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
        {@const has = holding.has(p.id)}
        <button class="item" aria-pressed={has} onclick={() => pick(p.id)} title={has ? `In ${p.name}. Tap to remove it.` : undefined}>
          <Mosaic covers={p.covers} size={36} radius={5} />
          <span class="text"><span class="ellipsis">{p.name}</span><span class="muted n">{has ? 'Added. Tap to remove' : `${p.count} ${p.count === 1 ? 'song' : 'songs'}`}</span></span>
          {#if has}<span class="check" aria-hidden="true"><Icon name="check" size={16} /></span>{/if}
        </button>
      {:else}
        <p class="muted empty">No playlists yet. Name one above.</p>
      {/each}
    </div>
    {#if single?.artists[0]}
      <button class="item block" onclick={() => single && blockArtist(single)}>
        <span class="text"><span class="ellipsis">Don't play {single.artists[0]}</span><span class="muted n">in radio and quick picks</span></span>
      </button>
    {/if}
  </div>
{/if}

<style>
  .backdrop {
    position: fixed;
    inset: 0;
    z-index: 50;
    cursor: default;
  }
  .menu {
    position: fixed;
    z-index: 51;
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
  .check {
    margin-left: auto;
    width: 24px;
    height: 24px;
    flex: none;
    border-radius: 50%;
    display: flex;
    align-items: center;
    justify-content: center;
    background: var(--accent, var(--accent-default));
    color: var(--on-accent);
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
  .block {
    border-top: 1px solid var(--line, rgb(255 255 255 / 0.08));
    border-radius: 0 0 var(--r-md) var(--r-md);
    margin-top: 4px;
    padding-left: 10px;
  }
  .empty {
    font-size: 13px;
    padding: 4px;
    margin: 0;
  }
</style>
