<script lang="ts">
  import { api, type Playlist } from '../api'
  import { downloadLabel, downloadOf, go, layout, playList, prefetchOnHover, refreshPlaylists, showToast, ui } from '../state.svelte'
  import BackButton from '../components/BackButton.svelte'
  import DownloadButton from '../components/DownloadButton.svelte'
  import Icon from '../components/Icon.svelte'
  import Mosaic from '../components/Mosaic.svelte'
  import TrackRow from '../components/TrackRow.svelte'

  let pl = $state<Playlist | null>(null)
  const dl = $derived(pl ? downloadOf(`playlist:${pl.id}`) : null)
  let missing = $state(false)
  let editing = $state(false)
  let draft = $state('')
  let confirmDelete = $state(false)
  let dragFrom = $state<number | null>(null)
  let dragOver = $state<number | null>(null)

  $effect(() => {
    const id = ui.playlistId
    if (id != null) api.syncPlaylist(id).catch(() => {})
  })
  $effect(() => {
    const id = ui.playlistId
    void ui.synced
    if (id == null) return
    confirmDelete = false
    api.playlist(id).then((p) => {
      pl = p
      missing = !p
      if (p && ui.renameNext) {
        ui.renameNext = false
        startRename()
      }
    })
  })

  const summary = $derived(ui.playlists.find((p) => p.id === pl?.id))
  const totalMin = $derived(Math.round((pl?.tracks ?? []).reduce((s, t) => s + (t.duration ?? 0), 0) / 60))

  function startRename() {
    if (!pl) return
    draft = pl.name
    editing = true
    queueMicrotask(() => {
      const el = document.getElementById('pl-name') as HTMLInputElement | null
      el?.focus()
      el?.select()
    })
  }
  async function saveName() {
    if (!pl || !editing) return
    editing = false
    const name = draft.trim() || 'New playlist'
    if (name === pl.name) return
    pl.name = name
    await api.renamePlaylist(pl.id, name)
    refreshPlaylists()
  }

  function shuffled() {
    const t = [...(pl?.tracks ?? [])]
    for (let i = t.length - 1; i > 0; i--) {
      const j = Math.floor(Math.random() * (i + 1))
      ;[t[i], t[j]] = [t[j], t[i]]
    }
    return t
  }

  async function remove(index: number) {
    if (!pl) return
    const [t] = pl.tracks.splice(index, 1)
    await api.removeFromPlaylist(pl.id, index)
    refreshPlaylists()
  }

  async function del() {
    if (!pl) return
    const name = pl.name
    await api.deletePlaylist(pl.id)
    await refreshPlaylists()
    showToast(`Deleted ${name}`)
    go('home')
  }

  async function hide() {
    if (!pl) return
    const name = pl.name
    await api.hidePlaylist(pl.id)
    await refreshPlaylists()
    showToast(`Hid ${name}. Show it again in Settings.`)
    go('home')
  }

  async function drop(to: number) {
    const from = dragFrom
    dragFrom = dragOver = null
    if (!pl || from == null || from === to) return
    const [t] = pl.tracks.splice(from, 1)
    pl.tracks.splice(to, 0, t)
    await api.moveInPlaylist(pl.id, from, to)
    refreshPlaylists()
  }
</script>

<div class="page scroll">
  <!-- Phones reach playlists from the Library list, so they get Back; the desktop has the sidebar. -->
  {#if layout.phone}<BackButton />{/if}
  {#if missing}
    <p class="muted">This playlist no longer exists.</p>
  {:else if pl}
    <header>
      <Mosaic covers={summary?.covers ?? []} size={layout.phone ? 240 : 168} radius={14} />
      <div class="info">
        {#if editing}
          <label for="pl-name" class="sr">Playlist name</label>
          <input id="pl-name" class="name-input" bind:value={draft} maxlength="100"
            onblur={saveName} onkeydown={(e) => {
              if (e.key === 'Enter') saveName()
              if (e.key === 'Escape') editing = false
            }} />
        {:else}
          <button class="name" onclick={startRename} title="Rename">{pl.name}</button>
        {/if}
        <p class="muted meta">
          {pl.tracks.length} {pl.tracks.length === 1 ? 'song' : 'songs'}{totalMin ? `, about ${totalMin} min` : ''}
        </p>
        {#if layout.phone && dl}
          {@const whole = dl.done >= dl.total}
          <div class="dlstatus" class:whole>
            <span class="trim">{downloadLabel(`playlist:${pl.id}`)}</span>
            {#if !whole}<span class="bar"><span style:width="{(dl.done / Math.max(dl.total, 1)) * 100}%"></span></span>{/if}
          </div>
        {/if}
        <div class="actions">
          <button class="primary" disabled={!pl.tracks.length} use:prefetchOnHover={pl.tracks[0]?.id} onclick={() => pl && playList(pl.tracks, 0)}>
            <Icon name="play" size={16} /><span class="trim">Play</span>
          </button>
          <button class="secondary" disabled={!pl.tracks.length} onclick={() => playList(shuffled(), 0)}>
            <Icon name="shuffle" size={16} /><span class="trim">Shuffle</span>
          </button>
          {#if pl.tracks.length}<DownloadButton target={`playlist:${pl.id}`} compact={layout.phone} />{/if}
          {#if confirmDelete}
            <span class="confirm">
              {#if pl.synced}
                Remove “{pl.name}”?
                <button class="secondary" onclick={hide} title="It stays on your YouTube Music account"><span class="trim">Hide in hum</span></button>
                <button class="danger" onclick={del}><span class="trim">Delete from YouTube Music</span></button>
              {:else}
                Delete “{pl.name}”?
                <button class="danger" onclick={del}><span class="trim">Delete</span></button>
              {/if}
              <button class="secondary" onclick={() => (confirmDelete = false)}><span class="trim">Keep</span></button>
            </span>
          {:else}
            <button class="icon-btn" aria-label={pl.synced ? 'Remove playlist' : 'Delete playlist'} title={pl.synced ? 'Remove playlist' : 'Delete playlist'} onclick={() => (confirmDelete = true)}>
              <Icon name="trash" size={18} />
            </button>
          {/if}
        </div>
      </div>
    </header>

    {#if pl.tracks.length}
      <div class="list">
        {#each pl.tracks as t, i (t.id)}
          <div
            class="drag"
            class:over={dragOver === i && dragFrom !== i}
            class:dragging={dragFrom === i}
            draggable="true"
            role="listitem"
            ondragstart={(e) => {
              dragFrom = i
              e.dataTransfer?.setData('text/plain', String(i))
              if (e.dataTransfer) e.dataTransfer.effectAllowed = 'move'
            }}
            ondragover={(e) => {
              e.preventDefault()
              dragOver = i
            }}
            ondragleave={() => dragOver === i && (dragOver = null)}
            ondrop={(e) => {
              e.preventDefault()
              drop(i)
            }}
            ondragend={() => (dragFrom = dragOver = null)}
          >
            <span class="grip" aria-hidden="true"><Icon name="grip" size={16} /></span>
            <div class="rowwrap">
              <TrackRow track={t} onplay={() => pl && playList(pl.tracks, i)}>
                {#snippet actions()}
                  <button class="tool" aria-label={`Remove ${t.title} from this playlist`} title="Remove from playlist" onclick={() => remove(i)}>
                    <Icon name="minus" size={18} />
                  </button>
                {/snippet}
              </TrackRow>
            </div>
          </div>
        {/each}
      </div>
    {:else}
      <div class="empty">
        <p>This playlist is empty.</p>
        <p class="muted">Use the <strong>+</strong> next to any song, or right-click it, to add it here.</p>
        <button class="primary" onclick={() => go('search')}><Icon name="search" size={16} /><span class="trim">Find songs</span></button>
      </div>
    {/if}
  {/if}
</div>

<style>
  .page {
    height: 100%;
    padding: 28px 32px 40px;
    display: flex;
    flex-direction: column;
    gap: 24px;
  }
  .sr {
    position: absolute;
    left: -9999px;
  }
  header {
    display: flex;
    gap: 24px;
    align-items: flex-end;
    flex-wrap: wrap;
  }
  .info {
    display: flex;
    flex-direction: column;
    gap: 8px;
    min-width: 0;
    flex: 1;
  }
  /* Phone: the cover centered and large under the back button, the name and buttons below
     at full width (side by side, they were squeezed into a narrow column). */
  @media (max-width: 640px) {
    header {
      flex-direction: column;
      align-items: stretch;
      margin-top: 36px;
      gap: 20px;
    }
    header > :global(.mosaic) {
      align-self: center;
    }
    .info {
      flex: none;
      gap: 14px;
    }
    /* Trimmed to the letters: the title's line box left more room under it than the song
       count had above the buttons, so the count sat low. */
    .name,
    .meta,
    .dlstatus > span {
      text-box: trim-both cap alphabetic;
    }
    .name,
    .name-input {
      font-size: 26px;
    }
  }
  .name,
  .name-input {
    font-size: 40px;
    font-weight: 800;
    letter-spacing: -0.03em;
    line-height: 1.1;
    text-align: left;
    padding: 2px 6px;
    margin-left: -6px;
    border-radius: var(--r-sm);
  }
  .name:hover {
    background: var(--raised);
  }
  .name-input {
    background: var(--surface);
    border: 2px solid var(--accent, var(--accent-default));
    outline: none;
    width: min(100%, 640px);
    user-select: text;
  }
  .meta {
    margin: 0;
  }
  /* The phone's download status (the button there is an icon): in the accent, with a bar while
     songs come in, so it doesn't read as one more line of plain text. */
  .dlstatus {
    display: flex;
    flex-direction: column;
    gap: 6px;
    color: var(--accent, var(--accent-default));
    font-size: 13px;
    font-weight: 700;
  }
  .dlstatus .bar {
    height: 4px;
    border-radius: 2px;
    background: var(--raised-2);
    overflow: hidden;
  }
  .dlstatus .bar > span {
    display: block;
    height: 100%;
    background: currentColor;
    border-radius: 2px;
    transition: width 0.3s;
  }
  .actions {
    display: flex;
    align-items: center;
    gap: 8px;
    flex-wrap: wrap;
  }
  .primary,
  .secondary,
  .danger {
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
  .danger {
    background: #c43c3c;
    color: #fff;
  }
  button:disabled {
    opacity: 0.4;
    cursor: default;
  }
  .confirm {
    display: inline-flex;
    align-items: center;
    gap: 8px;
    font-weight: 600;
  }
  .list {
    display: flex;
    flex-direction: column;
  }
  .drag {
    display: flex;
    align-items: center;
    border-radius: var(--r-md);
    border-top: 2px solid transparent;
  }
  .drag.over {
    border-top-color: var(--accent, var(--accent-default));
  }
  .drag.dragging {
    opacity: 0.4;
  }
  .grip {
    color: var(--muted);
    opacity: 0;
    cursor: grab;
    padding: 0 2px;
    display: flex;
  }
  .drag:hover .grip {
    opacity: 1;
  }
  .rowwrap {
    flex: 1;
    min-width: 0;
  }
  .empty {
    display: flex;
    flex-direction: column;
    gap: 8px;
    align-items: flex-start;
  }
  .empty p {
    margin: 0;
  }
</style>
