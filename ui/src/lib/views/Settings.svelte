<script lang="ts">
  import { api, img, onAccount, type AccountPlaylist, type Profile, type Quality } from '../api'
  import { player, refreshPlaylists, setQuality, setTheme, setAutoplay, showToast, ui, type ThemePref } from '../state.svelte'

  let lists = $state<AccountPlaylist[] | null>(null)
  $effect(() => {
    void ui.synced
    if (profile) api.accountPlaylists().then((l) => (lists = l)).catch(() => (lists = null))
    else lists = null
  })
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
          <h3>Your playlists on YouTube Music</h3>
          <div class="options">
            {#each lists as p (p.id)}
              <label class="opt">
                <input type="checkbox" checked={p.shown} onchange={(e) => setShown(p, e.currentTarget.checked)} />
                <span><span class="ol">{p.name}</span><span class="od">{p.count != null ? `${p.count} ${p.count === 1 ? 'song' : 'songs'}, ` : ''}{p.shown ? 'shown in hum' : 'hidden in hum, kept on YouTube Music'}</span></span>
              </label>
            {/each}
          </div>
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
    <label class="opt" class:sel={ui.autoplay}>
      <input type="checkbox" checked={ui.autoplay} onchange={(e) => setAutoplay(e.currentTarget.checked)} />
      <span><span class="ol">Autoplay radio</span><span class="od">When your queue runs out, keep going with songs like the last one</span></span>
    </label>
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
  .opt.sel {
    border-color: var(--accent, var(--accent-default));
  }
  .opt input {
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
  h3 {
    font-size: 14px;
    font-weight: 700;
    color: var(--muted);
    margin: 18px 0 8px;
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
