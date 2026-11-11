<script lang="ts">
  import type { Quality } from '../api'
  import { player, setQuality, setTheme, setAutoplay, ui, type ThemePref } from '../state.svelte'

  const themes: { v: ThemePref; label: string }[] = [
    { v: 'system', label: 'Match my system' },
    { v: 'dark', label: 'Dark' },
    { v: 'light', label: 'Light' },
  ]
  const qualities: { v: Quality; label: string; detail: string }[] = [
    { v: 'auto', label: 'Auto', detail: 'The best quality your connection handles' },
    { v: 'data_saver', label: 'Data saver', detail: 'About 50 kbps, roughly 0.4 MB per minute' },
    { v: 'normal', label: 'Normal', detail: 'About 130 kbps, roughly 1 MB per minute' },
    { v: 'high', label: 'High', detail: 'The best stream available' },
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
          <span><span class="ol">{q.label}</span><span class="od">{q.detail}</span></span>
        </label>
      {/each}
    </div>
    <p class="note">Takes effect from the next song.</p>
  </section>

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
    {#if player.lastTiming}
      <p class="note">Last song started {player.lastTiming.ms} ms after you pressed play{player.lastTiming.cached ? ' (it was loaded ahead of time)' : ''}.</p>
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
    font-size: 30px;
    font-weight: 800;
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
    border-radius: 12px;
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
  .od,
  .note {
    color: var(--muted);
    font-size: 13px;
  }
  .note {
    margin: 10px 0 0;
  }
</style>
