<script lang="ts">
  import { api, artistLine, type Track, img } from '../api'
  import { go, playList, playRadio, ui } from '../state.svelte'
  import Icon from '../components/Icon.svelte'
  import TrackRow from '../components/TrackRow.svelte'

  const seed = $derived(ui.recent[0] ?? null)
  let picks = $state<Track[]>([])
  let loading = $state(false)

  // Quick picks: the last song's radio ranked by your taste, with a few favorites mixed in.
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
      .quickPicks(s.id)
      .then((r) => {
        cache.set(s.id, r)
        if (seed?.id === s.id) picks = r
      })
      .catch(() => {})
      .finally(() => (loading = false))
  })

  let forgotten = $state<{ track: Track; last_played: number }[]>([])
  $effect(() => {
    api.forgotten(8).then((f) => (forgotten = f)).catch(() => {})
  })
  const monthYear = (ts: number) =>
    new Date(ts * 1000).toLocaleDateString('en-GB', { month: 'long', year: 'numeric' })

</script>

<div class="page scroll">
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

    {#if forgotten.length}
      <section>
        <div class="head">
          <h2>Haven't played these in a while</h2>
          <button class="link" onclick={() => playList(forgotten.map((f) => f.track), 0)}>Play all</button>
        </div>
        <div class="picks">
          {#each forgotten as f, i (f.track.id)}
            <TrackRow track={f.track} suffix={`, last played ${monthYear(f.last_played)}`}
              onplay={() => playList(forgotten.map((x) => x.track), i)} />
          {/each}
        </div>
      </section>
    {/if}

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
      <h1>Start with a song you like</h1>
      <p class="muted">hum keeps going with songs like it.</p>
      <button class="cta" onclick={() => go('search')}><span class="play"><Icon name="search" size={16} /></span><span class="trim">Search for a song</span></button>
    </section>
  {/if}
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
    font-size: 24px;
    font-weight: 700;
  }
  h2 {
    font-size: 18px;
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
    border-radius: var(--r-lg);
    background: var(--accent, var(--accent-default));
    color: var(--on-accent);
    font-weight: 700;
    font-size: 15px;
    transition: background 0.4s;
  }
  .play {
    width: 40px;
    height: 40px;
    border-radius: var(--r-md);
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
    border-radius: var(--r-md);
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
</style>
