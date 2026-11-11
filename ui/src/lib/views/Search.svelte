<script lang="ts">
  import { api, artistLinks, type Track, img } from '../api'
  import { forgetSearch, openAlbum, openArtist, playList, playRadio, rememberSearch, searches, ui, hoverPrefetch, hoverCancel } from '../state.svelte'
  import ArtistLinks from '../components/ArtistLinks.svelte'
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

  // A search is remembered once you act on its results, so half-typed queries don't pile up.
  const remember = () => rememberSearch(lastQuery)
  function again(q: string) {
    ui.query = q
    clearTimeout(timer)
    run(q)
  }

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
    {#if searches.list.length}
      <section class="recents">
        <div class="rhead">
          <h2>Recent searches</h2>
          <button class="link" onclick={() => forgetSearch(null)}>Clear</button>
        </div>
        <div class="chips">
          {#each searches.list as q (q)}
            <span class="chip">
              <button class="q" onclick={() => again(q)}><Icon name="clock" size={14} /><span class="trim">{q}</span></button>
              <button class="x" aria-label={`Remove ${q} from recent searches`} onclick={() => forgetSearch(q)}><Icon name="x" size={12} /></button>
            </span>
          {/each}
        </div>
      </section>
    {:else}
      <p class="muted hint">Search for a song or an artist. Every song can start an endless radio.</p>
    {/if}
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
            <div class="muted"><ArtistLinks artists={artistLinks(top)} />{#if top.album} — {#if top.album_id}<button class="inline-link" onclick={() => (remember(), openAlbum(top.album_id))}>{top.album}</button>{:else}{top.album}{/if}{/if}</div>
          </div>
          <div class="actions">
            <button class="primary" onclick={() => (remember(), playList(results, 0))}><Icon name="play" size={16} /><span class="trim">Play</span></button>
            <button class="secondary" onclick={() => (remember(), playRadio(top))}><Icon name="radio" size={16} /><span class="trim">Radio</span></button>
            {#if top.artist_id}<button class="secondary" onclick={() => (remember(), openArtist(top.artist_id))}><span class="trim">{top.artists[0]}</span></button>{/if}
          </div>
        </div>
      </section>
      <section class="songs">
        <h2>Songs</h2>
        <div class="list">
          {#each rest as t, i (t.id)}
            <TrackRow track={t} onplay={() => (remember(), playList(results, i + 1))} />
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
    /* Plus Jakarta Sans has a tall ascent, so centered text sits ~2px low next to the icon.
       Uneven padding lifts it by 1.5px, centering it between its cap height and x-height. */
    padding: 0 0 3px;
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
  .rhead {
    display: flex;
    align-items: center;
    justify-content: space-between;
    max-width: 720px;
  }
  .rhead h2 {
    margin: 0;
  }
  .link {
    color: var(--muted);
    font-weight: 600;
    min-height: 44px;
  }
  .link:hover {
    color: var(--text);
  }
  .chips {
    display: flex;
    flex-wrap: wrap;
    gap: 8px;
    max-width: 720px;
  }
  .chip {
    display: inline-flex;
    align-items: center;
    height: 40px;
    border-radius: 999px;
    background: var(--raised);
  }
  .chip:hover {
    background: var(--raised-2);
  }
  .chip .q {
    display: inline-flex;
    align-items: center;
    gap: 8px;
    height: 100%;
    padding: 0 6px 0 14px;
    font-weight: 600;
    color: var(--text);
  }
  .chip .q :global(svg) {
    color: var(--muted);
  }
  .chip .x {
    width: 32px;
    height: 32px;
    margin-right: 4px;
    border-radius: 50%;
    display: flex;
    align-items: center;
    justify-content: center;
    color: var(--muted);
  }
  .chip .x:hover {
    color: var(--text);
    background: var(--raised);
  }
  .inline-link {
    display: inline;
    padding: 0;
    font: inherit;
    color: inherit;
  }
  .inline-link:hover,
  .inline-link:focus-visible {
    text-decoration: underline;
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
