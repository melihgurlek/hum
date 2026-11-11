<script lang="ts">
  import { api, type Quality } from '../api'
  import { player, setQuality, setTheme, setAutoplay, ui, type ThemePref } from '../state.svelte'

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
