<script lang="ts">
  import { api, img, onAccount, type AccountPlaylist, type Profile, type Quality } from '../api'
  import { player, quickPicksCache, refreshPlaylists, setAccountRadio, setQuality, setTheme, setAutoplay, setNormalize, showToast, ui, type ThemePref } from '../state.svelte'

  let lists = $state<AccountPlaylist[] | null>(null)
  $effect(() => {
    void ui.synced
    if (profile) api.accountPlaylists().then((l) => (lists = l)).catch(() => (lists = null))
    else lists = null
  })
  let choosing = $state(false)
  let filter = $state('')
  const shownCount = $derived(lists?.filter((p) => p.shown).length ?? 0)
  const filtered = $derived((lists ?? []).filter((p) => p.name.toLowerCase().includes(filter.trim().toLowerCase())))
  async function setShown(p: AccountPlaylist, shown: boolean) {
    p.shown = shown
    await api.setPlaylistShown(p.id, p.name, shown).catch((e) => showToast(String(e)))
    refreshPlaylists()
  }

  let profile = $state<Profile | null | undefined>(undefined)
  $effect(() => {
    api.account().then((p) => (profile = p)).catch(() => (profile = null))
    const off = onAccount((p) => (profile = p), (msg) => showToast(`Sign-in failed: ${msg}`))
    return () => void off.then((fns) => fns.forEach((f) => f()))
  })

  let used = $state<{ month: number; total: number } | null>(null)
  $effect(() => {
    api.dataUsed().then((d) => {
      used = d
      player.autoLow = d.auto_low
    }).catch(() => {})
  })
  let saved = $state<{ count: number; bytes: number } | null>(null)
  $effect(() => {
    api.savedSongs().then((s) => (saved = s)).catch(() => {})
  })
  let blocked = $state<{ key: string; name: string }[]>([])
  $effect(() => {
    api.blockedArtists().then((b) => (blocked = b)).catch(() => {})
  })
  async function unblock(key: string) {
    await api.unblockArtist(key).catch((e) => showToast(String(e)))
    blocked = blocked.filter((b) => b.key !== key)
    quickPicksCache.clear()
  }

  async function clearSaved() {
    await api.clearSavedSongs()
    saved = await api.savedSongs()
  }
  function size(bytes: number): string {
    if (bytes < 1e6) return `${Math.round(bytes / 1e3)} KB`
    if (bytes < 1e9) return `${Math.round(bytes / 1e6)} MB`
    return `${(bytes / 1e9).toFixed(1)} GB`
  }
  const month = new Date().toLocaleDateString('en-GB', { month: 'long' })

  const themes: { v: ThemePref; label: string }[] = [
    { v: 'system', label: 'Match my system' },
    { v: 'dark', label: 'Dark' },
    { v: 'light', label: 'Light' },
  ]
  const qualities: { v: Quality; label: string; detail: string }[] = [
    { v: 'auto', label: 'Auto', detail: 'High, dropping to Data saver when your connection can’t keep up' },
    { v: 'high', label: 'High', detail: 'About 130 kbps, roughly 1 MB per minute' },
    { v: 'data_saver', label: 'Data saver', detail: 'About 50 kbps, roughly 0.4 MB per minute' },
  ]
</script>

<div class="page scroll">
  <h1>Settings</h1>

  {#if profile !== undefined}
    <section>
      <h2>YouTube Music account</h2>
      {#if profile}
        <div class="account">
          {#if profile.photo}<img class="avatar" src={img(profile.photo)} alt="" />{:else}<div class="avatar"></div>{/if}
          <span class="who"><span class="ol ellipsis">{profile.name}</span>{#if profile.email}<span class="od ellipsis">{profile.email}</span>{/if}</span>
          <button class="quiet" onclick={() => api.signOut()}>Sign out</button>
        </div>
        {#if lists?.length}
          <div class="account lists">
            <span class="who"><span class="ol">Playlists from YouTube Music</span><span class="od">{shownCount} of {lists.length} shown in hum; hidden ones stay on your account</span></span>
            <button class="quiet" onclick={() => (choosing = !choosing)} aria-expanded={choosing}>{choosing ? 'Done' : 'Choose'}</button>
          </div>
          {#if choosing}
            <div class="picker">
              {#if lists.length > 8}
                <input class="filter" placeholder="Filter playlists" bind:value={filter} aria-label="Filter playlists" />
              {/if}
              <div class="rows scroll">
                {#each filtered as p (p.id)}
                  <label class="row">
                    <input type="checkbox" checked={p.shown} onchange={(e) => setShown(p, e.currentTarget.checked)} />
                    <span class="ellipsis">{p.name}</span>
                    {#if p.count != null}<span class="od count">{p.count}</span>{/if}
                  </label>
                {:else}
                  <p class="od none">No playlist matches</p>
                {/each}
              </div>
            </div>
          {/if}
        {/if}
      {:else}
        <div class="account">
          <span class="who"><span class="ol">Not signed in</span><span class="od">Sign in to bring your liked songs and playlists from YouTube Music</span></span>
          <button class="filled" onclick={() => api.signIn().catch((e) => showToast(String(e)))}>Sign in with Google</button>
        </div>
      {/if}
    </section>
  {/if}

  <section>
    <h2>Sound quality</h2>
    <div class="options" role="radiogroup" aria-label="Sound quality">
      {#each qualities as q}
        <label class="opt" class:sel={ui.quality === q.v}>
          <input type="radio" name="quality" checked={ui.quality === q.v} onchange={() => setQuality(q.v)} />
          <span>
            <span class="ol">{q.label}{#if q.v === 'auto' && ui.quality === 'auto' && player.autoLow}<span class="pill">On Data saver now</span>{/if}</span>
            <span class="od">{q.v === 'auto' && ui.quality === 'auto' && player.autoLow
              ? 'Your connection was too slow for High. Auto tries High again after 15 calm minutes.'
              : q.detail}</span>
          </span>
        </label>
      {/each}
    </div>
  </section>

  {#if used}
    <section>
      <h2>Data used</h2>
      <div class="usage">
        <div><span class="num">{size(used.month)}</span><span class="od">in {month}</span></div>
        <div><span class="num">{size(used.total)}</span><span class="od">in all</span></div>
      </div>
      {#if saved}
        <div class="account lists">
          <span class="who"><span class="ol">Saved songs: {saved.count} {saved.count === 1 ? 'song' : 'songs'}, {size(saved.bytes)}</span><span class="od">Songs you play to the end are kept on this device (up to 1 GB), so replays use no data</span></span>
          {#if saved.count}<button class="quiet" onclick={clearSaved}>Clear</button>{/if}
        </div>
      {/if}
    </section>
  {/if}

  <section>
    <h2>Appearance</h2>
    <div class="options" role="radiogroup" aria-label="Theme">
      {#each themes as t}
        <label class="opt" class:sel={ui.theme === t.v}>
          <input type="radio" name="theme" checked={ui.theme === t.v} onchange={() => setTheme(t.v)} />
          <span class="ol">{t.label}</span>
        </label>
      {/each}
    </div>
  </section>

  <section>
    <h2>Playback</h2>
    <div class="options">
      <label class="opt" class:sel={ui.autoplay}>
        <input class="switch" type="checkbox" checked={ui.autoplay} onchange={(e) => setAutoplay(e.currentTarget.checked)} />
        <span><span class="ol">Autoplay radio</span><span class="od">When your queue runs out, keep going with songs like the last one</span></span>
      </label>
      <label class="opt" class:sel={ui.normalize}>
        <input class="switch" type="checkbox" checked={ui.normalize} onchange={(e) => setNormalize(e.currentTarget.checked)} />
        <span><span class="ol">Even out loudness</span><span class="od">Turns down songs mastered louder than most, as YouTube does, so you don't reach for the volume</span></span>
      </label>
      {#if profile}
        <label class="opt" class:sel={ui.accountRadio}>
          <input class="switch" type="checkbox" checked={ui.accountRadio} onchange={(e) => setAccountRadio(e.currentTarget.checked)} />
          <span><span class="ol">Radio from my YouTube Music account</span><span class="od">About half the songs change, picked for your account; off gives the radio everyone gets</span></span>
        </label>
      {/if}
    </div>
    {#if blocked.length}
      <div class="blocked">
        <span class="od">Artists you blocked: not played in radio or quick picks</span>
        {#each blocked as b (b.key)}
          <div class="account lists">
            <span class="who"><span class="ol">{b.name}</span></span>
            <button class="quiet" onclick={() => unblock(b.key)}>Allow again</button>
          </div>
        {/each}
      </div>
    {/if}
  </section>
</div>

<style>
  .page {
    height: 100%;
    padding: 28px 32px 40px;
    display: flex;
    flex-direction: column;
    gap: 28px;
    max-width: 720px;
  }
  h1 {
    font-size: 24px;
    font-weight: 700;
  }
  h2 {
    font-size: 18px;
    font-weight: 700;
    margin-bottom: 12px;
  }
  .options {
    display: flex;
    flex-direction: column;
    gap: 6px;
  }
  .opt {
    display: flex;
    align-items: center;
    gap: 14px;
    padding: 12px 16px;
    border-radius: var(--r-md);
    background: var(--surface);
    border: 2px solid transparent;
    cursor: pointer;
    min-height: 52px;
  }
  /* Switches sit at the end of their row, as on a phone's settings. */
  .opt:has(.switch) > span {
    flex: 1;
  }
  .opt .switch {
    order: 2;
  }
  .opt.sel {
    border-color: var(--accent, var(--accent-default));
  }
  .opt input:not(.switch) {
    accent-color: var(--accent, var(--accent-default));
    width: 18px;
    height: 18px;
    flex: none;
  }
  .opt > span {
    display: flex;
    flex-direction: column;
  }
  .ol {
    font-weight: 600;
  }
  .od {
    color: var(--muted);
    font-size: 13px;
  }
  .pill {
    margin-left: 8px;
    padding: 2px 8px;
    border-radius: 999px;
    background: var(--raised-2);
    font-size: 12px;
    font-weight: 700;
    vertical-align: 1px;
  }
  .lists {
    margin-top: 6px;
  }
  .picker {
    margin-top: 6px;
    padding: 8px;
    border-radius: var(--r-md);
    background: var(--surface);
  }
  .filter {
    width: 100%;
    height: 36px;
    padding: 0 12px;
    margin-bottom: 6px;
    border-radius: var(--r-sm);
    background: var(--raised);
    color: var(--text);
  }
  .rows {
    max-height: 320px;
    overflow-y: auto;
  }
  .row {
    display: flex;
    align-items: center;
    gap: 12px;
    height: 40px;
    padding: 0 8px;
    border-radius: var(--r-sm);
    cursor: pointer;
  }
  .row:hover {
    background: var(--raised);
  }
  .row input {
    accent-color: var(--accent, var(--accent-default));
    width: 16px;
    height: 16px;
    flex: none;
  }
  .row .ellipsis {
    flex: 1;
    min-width: 0;
  }
  .count {
    font-variant-numeric: tabular-nums;
  }
  .none {
    padding: 8px;
  }
  .account {
    display: flex;
    align-items: center;
    gap: 14px;
    padding: 12px 16px;
    border-radius: var(--r-md);
    background: var(--surface);
    min-height: 64px;
  }
  .avatar {
    width: 40px;
    height: 40px;
    border-radius: 50%;
    background: var(--raised-2);
    flex: none;
  }
  .who {
    flex: 1;
    min-width: 0;
    display: flex;
    flex-direction: column;
  }
  .blocked {
    display: flex;
    flex-direction: column;
    gap: 8px;
    margin-top: 16px;
  }
  .filled,
  .quiet {
    flex: none;
    height: 40px;
    padding: 0 18px;
    border-radius: 999px;
    font-weight: 700;
  }
  .filled {
    background: var(--accent, var(--accent-default));
    color: var(--on-accent);
  }
  .quiet {
    color: var(--muted);
  }
  .quiet:hover {
    color: var(--text);
    background: var(--raised);
  }
  .usage {
    display: flex;
    gap: 12px;
  }
  .usage > div {
    flex: 1;
    display: flex;
    flex-direction: column;
    gap: 2px;
    padding: 14px 16px;
    border-radius: var(--r-md);
    background: var(--surface);
  }
  .num {
    font-size: 22px;
    font-weight: 800;
    letter-spacing: -0.02em;
    font-variant-numeric: tabular-nums;
  }
</style>
