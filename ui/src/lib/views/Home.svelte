<script lang="ts">
  import { api, artistLine, type Track, img } from '../api'
  import { go, playList, playRadio, ui } from '../state.svelte'
  import Icon from '../components/Icon.svelte'
  import TrackRow from '../components/TrackRow.svelte'

  const seed = $derived(ui.recent[0] ?? null)
  let picks = $state<Track[]>([])
  let loading = $state(false)

  // Quick picks: the radio of the last song you played, minus songs you just heard.
  // Cached for the session so returning to Home is instant.
  const cache = new Map<string, Track[]>()
  $effect(() => {
    const s = seed
    if (!s) return
    if (cache.has(s.id)) {
      picks = cache.get(s.id)!
      return
    }
    loading = true
    api
      .radio(s.id)
      .then((r) => {
        const heard = new Set(ui.recent.slice(0, 10).map((t) => t.id))
        const fresh = r.filter((t) => !heard.has(t.id)).slice(0, 12)
        cache.set(s.id, fresh)
        if (seed?.id === s.id) picks = fresh
      })
      .catch(() => {})
      .finally(() => (loading = false))
  })

  const moods = ['Türkçe rock', 'Chill', 'Focus', 'Workout', 'Late night', '2000s alternative']
  function mood(m: string) {
    ui.query = m
    go('search')
  }

  const hour = new Date().getHours()
  const greeting = hour < 5 ? 'Up late' : hour < 12 ? 'Good morning' : hour < 18 ? 'Good afternoon' : 'Good evening'
</script>

<div class="page scroll">
  <h1>{greeting}</h1>

  {#if seed}
    <div class="cta-row">
      <button class="cta" onclick={() => playRadio(seed)}>
        <span class="play"><Icon name="play" size={16} /></span>
        <span class="trim">Keep going with {seed.artists[0] ?? seed.title} radio</span>
      </button>
    </div>

    <section>
      <div class="head">
        <h2>Quick picks</h2>
        {#if picks.length}<button class="link" onclick={() => playList(picks, 0)}>Play all</button>{/if}
      </div>
      {#if loading && !picks.length}
        <p class="muted">Finding songs like {seed.title}…</p>
      {:else}
        <div class="picks">
          {#each picks as t, i (t.id)}
            <TrackRow track={t} onplay={() => playList(picks, i)} />
          {/each}
        </div>
      {/if}
    </section>

    <section>
      <h2>Recently played</h2>
      <div class="tiles">
        {#each ui.recent.slice(0, 12) as t (t.id)}
          <button class="tile" onclick={() => playRadio(t)} title={`Radio from ${t.title}`}>
            {#if t.cover}<img class="cover art" src={img(t.cover)} alt="" loading="lazy" />{:else}<div class="cover art"></div>{/if}
            <span class="ellipsis tt">{t.title}</span>
            <span class="ellipsis muted ts">{artistLine(t)}</span>
          </button>
        {/each}
      </div>
    </section>
  {:else}
    <section class="empty">
      <h2>Start with a song you like</h2>
      <p class="muted">Play anything and hum keeps going with songs like it. Your home fills up as you listen.</p>
      <button class="cta" onclick={() => go('search')}><span class="play"><Icon name="search" size={16} /></span><span class="trim">Search for a song</span></button>
    </section>
  {/if}

  <section>
    <h2>Moods</h2>
    <div class="chips">
      {#each moods as m}<button class="chip" onclick={() => mood(m)}><span class="trim">{m}</span></button>{/each}
    </div>
  </section>
</div>

<style>
  .page {
    height: 100%;
    padding: 28px 32px 40px;
    display: flex;
    flex-direction: column;
    gap: 28px;
  }
  h1 {
    font-size: 30px;
    font-weight: 800;
  }
  h2 {
    font-size: 20px;
    font-weight: 700;
  }
  .head {
    display: flex;
    justify-content: space-between;
    align-items: baseline;
    margin-bottom: 8px;
  }
  section > h2 {
    margin-bottom: 12px;
  }
  .link {
    color: var(--muted);
    font-weight: 600;
    min-height: 44px;
  }
  .link:hover {
    color: var(--text);
  }
  .cta-row {
    display: flex;
    gap: 10px;
    flex-wrap: wrap;
  }
  .cta {
    display: inline-flex;
    align-items: center;
    gap: 12px;
    height: 56px;
    padding: 0 22px 0 8px;
    border-radius: 14px;
    background: var(--accent, var(--accent-default));
    color: var(--on-accent);
    font-weight: 700;
    font-size: 15px;
    transition: background 0.4s;
  }
  .play {
    width: 40px;
    height: 40px;
    border-radius: 10px;
    background: color-mix(in srgb, var(--on-accent) 18%, transparent);
    display: flex;
    align-items: center;
    justify-content: center;
  }
  .picks {
    display: grid;
    grid-template-columns: repeat(auto-fill, minmax(280px, 1fr));
    gap: 2px 16px;
  }
  .tiles {
    display: grid;
    grid-template-columns: repeat(auto-fill, minmax(150px, 1fr));
    gap: 20px;
  }
  .tile {
    display: flex;
    flex-direction: column;
    gap: 6px;
    text-align: left;
    min-width: 0;
  }
  .art {
    width: 100%;
    aspect-ratio: 1;
    border-radius: 10px;
  }
  .tt {
    font-weight: 600;
  }
  .ts {
    font-size: 13px;
    margin-top: -4px;
  }
  .empty {
    display: flex;
    flex-direction: column;
    gap: 12px;
    align-items: flex-start;
    max-width: 520px;
  }
  .empty p {
    margin: 0;
    font-size: 15px;
    line-height: 1.5;
  }
  .chips {
    display: flex;
    gap: 8px;
    flex-wrap: wrap;
  }
  .chip {
    height: 36px;
    padding: 0 16px;
    border-radius: 999px;
    background: var(--raised);
    font-weight: 600;
    font-size: 13px;
  }
  .chip:hover {
    background: var(--raised-2);
  }
</style>
