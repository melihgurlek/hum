<script lang="ts">
  import { api, artistLine, type Track, img } from '../api'
  import { playList, playRadio, ui, hoverPrefetch, hoverCancel } from '../state.svelte'
  import Icon from '../components/Icon.svelte'
  import TrackRow from '../components/TrackRow.svelte'

  let results = $state<Track[]>([])
  let searching = $state(false)
  let error = $state<string | null>(null)
  let lastQuery = ''
  let timer: ReturnType<typeof setTimeout> | undefined
  let input: HTMLInputElement

  // Results stay cached per query for this session, so going back is instant.
  const cache = new Map<string, Track[]>()

  async function run(q: string) {
    q = q.trim()
    if (!q || q === lastQuery) return
    lastQuery = q
    error = null
    if (cache.has(q)) {
      results = cache.get(q)!
      return
    }
    searching = true
    try {
      const r = await api.search(q)
      cache.set(q, r)
      if (q === lastQuery) results = r
    } catch (e) {
      if (q === lastQuery) error = 'Search failed. Check your connection and try again.'
    } finally {
      if (q === lastQuery) searching = false
    }
  }

  function onInput() {
    clearTimeout(timer)
    timer = setTimeout(() => run(ui.query), 300)
  }

  $effect(() => {
    input?.focus()
    if (ui.query) run(ui.query)
  })

  const top = $derived(results[0])
  const rest = $derived(results.slice(1))
</script>

<div class="page scroll">
  <div class="field">
    <Icon name="search" />
    <label for="q" class="sr">Search songs and artists</label>
    <input
      id="q"
      bind:this={input}
      bind:value={ui.query}
      oninput={onInput}
      onkeydown={(e) => e.key === 'Enter' && (clearTimeout(timer), run(ui.query))}
      placeholder="What do you want to hear?"
      autocomplete="off"
      spellcheck="false"
    />
    {#if ui.query}
      <button class="clear" aria-label="Clear search" onclick={() => ((ui.query = ''), (results = []), (lastQuery = ''), input.focus())}>
        <Icon name="x" size={14} />
      </button>
    {/if}
  </div>

  {#if error}
    <p class="muted">{error}</p>
  {:else if !ui.query}
    <p class="muted hint">Search for a song or an artist. Every song can start an endless radio.</p>
  {:else if searching && !results.length}
    <p class="muted">Searching…</p>
  {:else if top}
    <div class="grid">
      <section class="topcol">
        <h2>Top result</h2>
        <div class="top" role="group" aria-label={top.title}
          onmouseenter={() => hoverPrefetch(top.id)} onmouseleave={hoverCancel}>
          {#if top.cover}<img class="cover topart" src={img(top.cover)} alt="" />{:else}<div class="cover topart"></div>{/if}
          <div>
            <div class="toptitle">{top.title}</div>
            <div class="muted">{artistLine(top)}{top.album ? ` — ${top.album}` : ''}</div>
          </div>
          <div class="actions">
            <button class="primary" onclick={() => playList(results, 0)}><Icon name="play" size={16} />Play</button>
            <button class="secondary" onclick={() => playRadio(top)}><Icon name="radio" size={16} />Radio</button>
          </div>
        </div>
      </section>
      <section class="songs">
        <h2>Songs</h2>
        <div class="list">
          {#each rest as t, i (t.id)}
            <TrackRow track={t} onplay={() => playList(results, i + 1)} />
          {/each}
        </div>
      </section>
    </div>
  {:else}
    <p class="muted">Nothing found for “{ui.query}”. Try fewer words or check the spelling.</p>
  {/if}
</div>

<style>
  .page {
    height: 100%;
    padding: 24px 32px 40px;
    display: flex;
    flex-direction: column;
    gap: 24px;
  }
  .sr {
    position: absolute;
    left: -9999px;
  }
  .field {
    display: flex;
    align-items: center;
    gap: 10px;
    height: 48px;
    padding: 0 18px;
    border-radius: 14px;
    background: var(--surface);
    border: 2px solid var(--accent, var(--accent-default));
    color: var(--muted);
    max-width: 720px;
    flex: none;
  }
  .field input {
    flex: 1;
    border: 0;
    outline: none;
    background: transparent;
    font-size: 16px;
    color: var(--text);
    user-select: text;
  }
  .clear {
    width: 32px;
    height: 32px;
    border-radius: 50%;
    background: var(--raised);
    display: flex;
    align-items: center;
    justify-content: center;
  }
  .hint {
    font-size: 15px;
  }
  .grid {
    display: flex;
    flex-wrap: wrap;
    gap: 32px;
  }
  h2 {
    font-size: 18px;
    font-weight: 700;
    margin-bottom: 12px;
  }
  .topcol {
    flex: 1 1 320px;
    max-width: 420px;
  }
  .top {
    display: flex;
    flex-direction: column;
    gap: 16px;
    padding: 24px;
    border-radius: 20px;
    background: var(--surface);
  }
  .topart {
    width: 140px;
    height: 140px;
    border-radius: 12px;
  }
  .toptitle {
    font-size: 30px;
    font-weight: 800;
    letter-spacing: -0.03em;
    line-height: 1.1;
  }
  .actions {
    display: flex;
    gap: 8px;
    flex-wrap: wrap;
  }
  .primary,
  .secondary {
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
  .secondary {
    background: var(--raised);
  }
  .songs {
    flex: 999 1 420px;
    min-width: 0;
  }
  .list {
    display: flex;
    flex-direction: column;
  }
</style>
