<script lang="ts">
  import { api, artistLine, artistLinks, fmtTime, img, type TrackNote } from '../api'
  import { cubicOut } from 'svelte/easing'
  import { flip } from 'svelte/animate'
  import { closeNowPlaying, layout, player, ui, setAutoplay, setNpSheet, toggleLike, reason, openAddMenu, openAlbum, prefetchOnHover } from '../state.svelte'
  import ArtistLinks from '../components/ArtistLinks.svelte'
  import Icon from '../components/Icon.svelte'
  import Progress from '../components/Progress.svelte'
  import Lyrics from '../components/Lyrics.svelte'
  import SleepTimer from '../components/SleepTimer.svelte'
  import RepeatButton from '../components/RepeatButton.svelte'
  import ShuffleButton from '../components/ShuffleButton.svelte'

  // Remember which tab you left open.
  let tab = $state<'next' | 'lyrics'>((localStorage.getItem('hum.npTab') as 'next' | 'lyrics') ?? 'lyrics')
  $effect(() => {
    try {
      localStorage.setItem('hum.npTab', tab)
    } catch {}
  })

  // Up next is the whole queue: the songs already played above the current one, dimmed, so
  // tapping a song only moves the highlight and nothing jumps. The list keeps the current song
  // at its top (scroll up for what played), except right after you tap, so the row stays under
  // your finger.
  const rows = $derived.by(() => {
    const from = Math.max(0, player.index - 50)
    return player.queue.slice(from, player.index + 51).map((t, i) => ({ t, at: from + i }))
  })
  let queueEl = $state<HTMLElement>()
  let tapped: string | undefined
  function tapRow(at: number) {
    if (at === player.index) return
    tapped = player.queue[at].id
    api.jump(at)
  }
  function toCurrent(smooth: boolean) {
    const row = queueEl?.querySelector<HTMLElement>('.qrow.now')
    if (!queueEl || !row) return
    const top = queueEl.scrollTop + row.getBoundingClientRect().top - queueEl.getBoundingClientRect().top - 4
    queueEl.scrollTo({ top, behavior: smooth && motion ? 'smooth' : 'auto' })
  }
  // Opening Up next lands on the current song; a song that changes by itself scrolls there.
  $effect(() => {
    if (!queueEl) return
    queueMicrotask(() => toCurrent(false))
  })
  let shownId: string | undefined
  $effect(() => {
    const id = player.current?.id
    if (id === shownId) return
    const first = shownId === undefined
    shownId = id
    if (first || id === tapped) return
    queueMicrotask(() => toCurrent(true))
  })
  const motion = matchMedia('(prefers-reduced-motion: reduce)').matches ? 0 : 1

  // Why each song is in Up next, from your listening history.
  let notes = $state<Record<string, TrackNote>>({})
  $effect(() => {
    const ids = rows.map((r) => r.t.id)
    if (!ids.length) return
    api.trackNotes(ids).then((n) => (notes = n)).catch(() => {})
  })
  const why = (id: string) => reason(notes[id])

  // Whether this song is in any playlist, so + can show a check like the heart shows a like.
  // Re-asked when the add menu closes or the playlists change.
  let inPlaylist = $state(false)
  let askedFor: string | undefined
  $effect(() => {
    const id = player.current?.id
    void ui.playlists
    if (id !== askedFor) inPlaylist = false
    askedFor = id
    if (!id || ui.addMenu) return
    api.playlistsWith(id).then((r) => (inPlaylist = r.length > 0 && player.current?.id === id)).catch(() => {})
  })

  function close() {
    closeNowPlaying()
  }

  // Opening and closing: on a phone the screen slides up from the mini player and back down
  // (from wherever a drag left it); on desktop it rises a little and fades.
  function sheet(node: HTMLElement, { out = false } = {}) {
    if (!layout.phone) {
      return { duration: 200, easing: cubicOut, css: (t: number, u: number) => `opacity: ${t}; transform: translateY(${u * 24}px)` }
    }
    const h = node.clientHeight || innerHeight
    const from = out ? dragY : 0
    return {
      duration: out ? 240 : 320,
      easing: cubicOut,
      css: (_t: number, u: number) => `transform: translateY(${out ? from + u * (h - from) : u * h}px)`,
    }
  }

  // Phone gestures. Drag the player down to close it, or sideways to change song; drag the Up
  // next bar up to open its sheet, and the sheet down (from its top, or from a list already
  // scrolled to the top) to put it away. The first 10px pick the direction, so taps and the
  // progress bar keep working.
  let dragY = $state(0) // the whole screen, while closing it
  let sheetDy = $state(0) // the sheet, while moving it
  let artDx = $state(0) // the cover, while swiping to another song
  let artSettling = $state(false)
  let dragging = $state(false)
  let g: { x: number; y: number; t: number; mode: 'np' | 'sheet' } | null = null
  let axis: 'x' | 'y' | null = null
  let dragged = false
  function down(e: PointerEvent) {
    if (!layout.phone || e.pointerType === 'mouse') return
    const el = e.target as HTMLElement
    if (el.closest('.progress, .menu, .backdrop')) return
    // The screen's edges belong to Android's back gesture.
    if (e.clientX < 24 || e.clientX > innerWidth - 24) return
    const onSheetTop = !!el.closest('.grab, .panel .tabs')
    if (el.closest('.panel') && !onSheetTop) return
    g = { x: e.clientX, y: e.clientY, t: performance.now(), mode: onSheetTop ? 'sheet' : ui.npSheet ? 'sheet' : 'np' }
    if (g.mode === 'np' && ui.npSheet) g = null
    axis = null
    dragged = false
  }
  function move(e: PointerEvent) {
    if (!g) return
    const mx = e.clientX - g.x
    const my = e.clientY - g.y
    if (!axis && Math.hypot(mx, my) > 10) {
      axis = Math.abs(my) > Math.abs(mx) ? 'y' : 'x'
      try {
        ;(e.currentTarget as HTMLElement).setPointerCapture(e.pointerId)
      } catch {}
    }
    if (axis === 'x' && g.mode === 'np') {
      artSettling = false
      artDx = mx
      return
    }
    if (axis !== 'y') return
    dragging = true
    if (g.mode === 'np') dragY = Math.max(0, my)
    else sheetDy = ui.npSheet ? Math.max(0, my) : Math.min(0, my)
  }
  function up(e: PointerEvent) {
    if (!g) return
    const my = e.clientY - g.y
    const speed = my / Math.max(1, performance.now() - g.t) // px/ms, down is positive
    if (axis) dragged = true
    if (axis === 'x' && g.mode === 'np') {
      const mx = e.clientX - g.x
      const fast = Math.abs(mx) / Math.max(1, performance.now() - g.t) > 0.5
      if (Math.abs(mx) > innerWidth * 0.25 || fast) swipeSong(Math.sign(mx))
      else {
        artSettling = true
        artDx = 0
      }
    }
    if (axis === 'y') {
      if (g.mode === 'np') {
        if (my > 140 || speed > 0.6) close()
        else settle()
      } else if (ui.npSheet && (my > 100 || speed > 0.6)) {
        setNpSheet(false)
        settle()
      } else if (!ui.npSheet && (my < -60 || speed < -0.6)) {
        setNpSheet(true)
        settle()
      } else settle()
    }
    g = null
    axis = null
  }
  // The cover slides off, the next (or previous) song is shown at once, and its cover slides
  // in from the other side. Previous more than 3 s in restarts this song, as the button does.
  function swipeSong(dir: number) {
    artSettling = true
    artDx = dir * innerWidth
    setTimeout(() => {
      const target = dir < 0 ? player.queue[player.index + 1] : player.position > 3 ? player.current : player.queue[player.index - 1]
      if (target) player.current = target
      if (dir < 0) api.next()
      else api.previous()
      artSettling = false
      artDx = -dir * innerWidth * 0.3
      requestAnimationFrame(() => requestAnimationFrame(() => ((artSettling = true), (artDx = 0))))
    }, 120)
  }

  // A drag down inside Up next or the lyrics, with the list already at its top, moves the
  // sheet instead. Touch events, not pointer ones: the list scrolls natively, and only a
  // non-passive touchmove can take the gesture back from it.
  function sheetPull(node: HTMLElement) {
    let y0 = 0
    let t0 = 0
    let list: HTMLElement | null = null
    let pulling = false
    let dy = 0
    const start = (e: TouchEvent) => {
      const el = e.target as HTMLElement
      list = ui.npSheet && layout.phone ? el.closest<HTMLElement>('.scroll') : null
      y0 = e.touches[0].clientY
      t0 = performance.now()
      pulling = false
      dy = 0
    }
    const moveT = (e: TouchEvent) => {
      if (!list) return
      dy = e.touches[0].clientY - y0
      if (!pulling && dy > 6 && list.scrollTop <= 0) pulling = true
      if (!pulling) return
      e.preventDefault()
      dragging = true
      sheetDy = Math.max(0, dy)
    }
    const end = () => {
      if (pulling) {
        const speed = dy / Math.max(1, performance.now() - t0)
        if (dy > 100 || speed > 0.6) setNpSheet(false)
        settle()
      }
      list = null
      pulling = false
    }
    node.addEventListener('touchstart', start, { passive: true })
    node.addEventListener('touchmove', moveT, { passive: false })
    node.addEventListener('touchend', end)
    node.addEventListener('touchcancel', end)
    return {
      destroy() {
        node.removeEventListener('touchstart', start)
        node.removeEventListener('touchmove', moveT)
        node.removeEventListener('touchend', end)
        node.removeEventListener('touchcancel', end)
      },
    }
  }

  function settle() {
    dragging = false
    if (ui.nowPlayingOpen) dragY = 0
    sheetDy = 0
  }
  // Android's back gesture (or anything else) took the touch: put everything back, the
  // cover too.
  function cancel() {
    g = null
    axis = null
    settle()
    artSettling = true
    artDx = 0
  }
  function click(e: MouseEvent) {
    if (dragged) {
      e.stopPropagation()
      e.preventDefault()
      dragged = false
    }
  }
  function pickTab(t: 'next' | 'lyrics') {
    tab = t
    if (layout.phone) setNpSheet(true)
  }
</script>

<svelte:window onkeydown={(e) => e.key === 'Escape' && (ui.npSheet ? setNpSheet(false) : close())} />

<div class="np" class:dragging role="dialog" aria-label="Now playing" in:sheet|global out:sheet|global={{ out: true }}
  style:transform={dragY ? `translateY(${dragY}px)` : undefined}
  onpointerdown={down} onpointermove={move} onpointerup={up} onpointercancel={cancel} onclickcapture={click}>
  <header>
    <button class="round" onclick={close} aria-label="Close now playing"><Icon name="chevronDown" /></button>
    <div class="from">
      <span class="from-l">Playing from</span> <strong>{ui.autoplay ? 'Autoplay radio' : 'Your queue'}</strong>
    </div>
    <span class="spacer"></span>
    <!-- Phone: the player bar is hidden here, so its sleep timer moves up here. -->
    <span class="phone-only side-ctl right"><SleepTimer size={22} down /></span>
  </header>

  {#if player.current}
    {@const t = player.current}
    <div class="body">
      <section class="left" aria-label="Song">
        {#if t.cover}
          <img class="art" class:settling={artSettling} src={img(t.cover, true)} alt={`${t.album ?? t.title} cover`}
            style:transform={artDx ? `translateX(${artDx}px)` : undefined} style:opacity={artDx ? Math.max(0.3, 1 - Math.abs(artDx) / 500) : undefined} />
        {:else}
          <div class="art" class:settling={artSettling} style:transform={artDx ? `translateX(${artDx}px)` : undefined}></div>
        {/if}
        <div class="meta">
          <div class="titles">
            <!-- Phone drops the album line; the title takes you to the album instead. -->
            <h1>{#if t.album_id && layout.phone}<button class="title-link" onclick={() => openAlbum(t.album_id)}>{t.title}</button>{:else}{t.title}{/if}</h1>
            <div class="artist"><ArtistLinks artists={artistLinks(t)} /></div>
            {#if t.album}
              {#if t.album_id}<button class="album album-link" onclick={() => openAlbum(t.album_id)}>{t.album}</button>
              {:else}<div class="album">{t.album}</div>{/if}
            {/if}
          </div>
          <button class="heart" class:on={player.liked} aria-pressed={player.liked}
            aria-label={player.liked ? 'Remove from Liked songs' : 'Add to Liked songs'} onclick={toggleLike}>
            <Icon name={player.liked ? 'heart' : 'heartOutline'} size={24} />
          </button>
          <button class="heart" class:on={inPlaylist} aria-label={inPlaylist ? 'In a playlist. Add to another' : 'Add to playlist'}
            title={inPlaylist ? 'In a playlist' : 'Add to playlist'} onclick={(e) => openAddMenu([t], e)}>
            <Icon name={inPlaylist ? 'check' : 'plus'} size={24} />
          </button>
        </div>
        <!-- Desktop: the player bar below is part of this screen and has the seek bar and every
             control, so they aren't repeated here. -->
        {#if layout.phone}
        <div class="progress">
          <Progress tone="np" />
          <div class="times"><span>{fmtTime(player.position)}</span><span>{fmtTime(player.duration)}</span></div>
        </div>
        <div class="controls">
          <!-- Phone: the player bar is hidden here, so shuffle and repeat join this row. -->
          <span class="phone-only side-ctl"><ShuffleButton size={22} /></span>
          <button class="ctl" aria-label="Previous" onclick={() => api.previous()}><Icon name="prev" size={28} /></button>
          <button class="big" aria-label={player.paused ? 'Play' : 'Pause'} onclick={() => api.togglePause()}>
            <Icon name={player.paused ? 'play' : 'pause'} size={30} />
          </button>
          <button class="ctl" aria-label="Next" onclick={() => api.next()}><Icon name="next" size={28} /></button>
          <span class="phone-only side-ctl right"><RepeatButton size={22} /></span>
        </div>
        {/if}
      </section>

      <section class="panel" use:sheetPull class:open={ui.npSheet} class:dragging aria-label="Up next and lyrics" style:--drag={`${sheetDy}px`}>
        <!-- Phone: the sheet's handle. Tap or drag it to pull the sheet up or put it away. -->
        <button class="grab phone-only" aria-label={ui.npSheet ? 'Hide up next and lyrics' : 'Show up next and lyrics'}
          aria-expanded={ui.npSheet} onclick={() => setNpSheet(!ui.npSheet)}><span></span></button>
        <div class="tabs" role="tablist">
          <button role="tab" aria-selected={tab === 'next'} class:sel={tab === 'next' && (ui.npSheet || !layout.phone)} onclick={() => pickTab('next')}><span class="trim">Up next</span></button>
          <button role="tab" aria-selected={tab === 'lyrics'} class:sel={tab === 'lyrics' && (ui.npSheet || !layout.phone)} onclick={() => pickTab('lyrics')}><span class="trim">Lyrics</span></button>
          <span class="spacer"></span>
          <label class="auto desk-only">
            Autoplay
            <input class="switch" type="checkbox" checked={ui.autoplay} onchange={(e) => setAutoplay(e.currentTarget.checked)} />
          </label>
        </div>

        {#if tab === 'next'}
          <!-- Phone: autoplay lives in the sheet, not on the bar that peeks below the player. -->
          <label class="auto phone-only auto-row">
            <span><strong>Autoplay radio</strong><span class="auto-d">Keep going with songs like these when the queue ends</span></span>
            <input class="switch" type="checkbox" checked={ui.autoplay} onchange={(e) => setAutoplay(e.currentTarget.checked)} />
          </label>
          <div class="queue scroll" bind:this={queueEl}>
            {#each rows as { t: q, at } (q.id)}
              <button
                class="qrow"
                class:now={at === player.index}
                class:past={at < player.index}
                aria-current={at === player.index ? 'true' : undefined}
                use:prefetchOnHover={q.id}
                onclick={() => tapRow(at)}
                animate:flip={{ duration: 260 * motion, easing: cubicOut }}
              >
                {#if q.thumb}<img class="qart" src={img(q.thumb)} alt="" loading="lazy" />{:else}<div class="qart"></div>{/if}
                <span class="qtext"><span class="ellipsis qt">{q.title}</span><span class="ellipsis qs">{artistLine(q)}</span></span>
                <span class="why">{at === player.index ? 'Playing' : at > player.index ? why(q.id) : ''}</span>
                <span class="qd">{fmtTime(q.duration)}</span>
              </button>
            {/each}
            {#if player.index + 1 >= player.queue.length}
              <p class="empty">
                {#if ui.autoplay}Finding songs like this one…{:else}Nothing queued. Turn on autoplay radio to keep the music going.{/if}
              </p>
            {/if}
          </div>
        {:else}
          <Lyrics />
        {/if}
      </section>
    </div>
  {:else}
    <p class="empty">Nothing is playing.</p>
  {/if}
</div>

<style>
  .np {
    position: absolute;
    inset: 0;
    z-index: 10;
    display: flex;
    flex-direction: column;
    color: var(--np-text);
    background: radial-gradient(1100px 700px at 22% 30%, var(--np-bg, var(--np-bg-default)) 0%, var(--np-bg-2, var(--np-bg-2-default)) 100%);
    transition: background 0.4s;
  }
  header {
    display: flex;
    align-items: center;
    gap: 12px;
    padding: 16px 24px;
  }
  .round {
    width: 44px;
    height: 44px;
    border-radius: 50%;
    background: color-mix(in srgb, black 22%, transparent);
    display: flex;
    align-items: center;
    justify-content: center;
  }
  .from {
    font-size: 13px;
    color: var(--np-muted);
  }
  .from strong {
    color: var(--np-text);
    font-weight: 600;
  }
  .spacer {
    flex: 1;
  }
  /* Two columns that fill exactly the height Now Playing has, so the panel ends above the
     player bar however tall the window is. (A wrapping flex row sized the panel by its
     content plus a 100vh guess, and it ran ~40px under the bar.) */
  .body {
    flex: 1;
    min-height: 0;
    display: grid;
    grid-template-columns: minmax(280px, 440px) minmax(0, 1fr);
    gap: 48px;
    padding: 4px 56px 32px;
  }
  .left {
    min-height: 0;
    display: flex;
    flex-direction: column;
    gap: 20px;
    justify-content: safe center;
  }
  /* The cover gives way first when the window is short: it takes the height left after the
     title and controls (its width follows), and never more than the column's width. */
  .art {
    flex: 0 1 440px;
    min-height: 96px;
    width: auto;
    max-width: 100%;
    align-self: flex-start;
    aspect-ratio: 1;
    border-radius: var(--r-lg);
    object-fit: cover;
    background: color-mix(in srgb, black 25%, transparent);
    box-shadow: 0 24px 48px rgba(0, 0, 0, 0.35);
  }
  .meta {
    display: flex;
    align-items: flex-start;
    gap: 12px;
  }
  .titles {
    flex: 1;
    min-width: 0;
  }
  .heart {
    width: 44px;
    height: 44px;
    border-radius: 50%;
    display: flex;
    align-items: center;
    justify-content: center;
    color: var(--np-muted);
    flex: none;
  }
  .heart.on {
    color: var(--np-text);
  }
  h1 {
    font-size: 30px;
    font-weight: 800;
    line-height: 1.1;
  }
  .artist {
    font-size: 18px;
    color: var(--np-muted);
    margin-top: 4px;
  }
  .album {
    font-size: 14px;
    color: var(--np-muted);
  }
  .title-link {
    padding: 0;
    font: inherit;
    color: inherit;
    text-align: left;
  }
  .album-link {
    display: block;
    padding: 0;
    text-align: left;
    font: inherit;
    font-size: 14px;
  }
  /* Hover only where there's a pointer: after a tap a touchscreen keeps :hover, which left
     the underline stuck. */
  @media (hover: hover) {
    .album-link:hover {
      text-decoration: underline;
    }
  }
  .album-link:focus-visible {
    text-decoration: underline;
  }
  .progress {
    display: flex;
    flex-direction: column;
    gap: 4px;
  }
  /* The column's 20px gap is right around the cover, too loose between the titles and the bar. */
  .meta + .progress {
    margin-top: -8px;
  }
  /* The bar's own flex: 1 is for the player bar's row; in this column it collapsed the bar
     to 0px tall, so the times sat on the line and the knob over them. */
  .progress :global(.track) {
    flex: none;
  }
  .times {
    display: flex;
    justify-content: space-between;
    font-size: 13px;
    color: var(--np-muted);
    font-variant-numeric: tabular-nums;
  }
  .controls {
    display: flex;
    align-items: center;
    justify-content: center;
    gap: 28px;
  }
  .ctl {
    width: 52px;
    height: 52px;
    display: flex;
    align-items: center;
    justify-content: center;
  }
  .big {
    width: 72px;
    height: 72px;
    border-radius: 50%;
    background: var(--np-text);
    color: var(--np-bg-2, var(--np-bg-2-default));
    display: flex;
    align-items: center;
    justify-content: center;
  }
  .panel {
    flex: 999 1 420px;
    min-width: 0;
    min-height: 0;
    display: flex;
    flex-direction: column;
    gap: 14px;
    background: color-mix(in srgb, black 20%, transparent);
    border-radius: var(--r-lg);
    padding: 18px 20px;
  }
  /* Narrow windows: stack, and let the whole screen scroll instead. */
  @media (max-width: 900px) {
    /* A flex column, not a one-column grid: with min-height: 0 on the items, the grid's rows
       shrank to fit the screen and the panel slid over the cover. */
    .body {
      display: flex;
      flex-direction: column;
      overflow-y: auto;
      padding: 4px 24px 24px;
      gap: 24px;
    }
    .left {
      flex: none;
      max-width: 440px;
    }
    .panel {
      flex: none;
      height: 70vh;
    }
  }
  .phone-only {
    display: none;
  }
  /* Phone: Now Playing is its own full screen over the tab bar (see the block below). */
  @media (max-width: 640px) {
    /* One screen, no scrolling: the cover gives way on short phones, and Up next and lyrics
       wait in a sheet whose bar is pinned at the bottom. */
    /* clip, not hidden: a hidden box can still be scrolled by code, which once slid the whole
       player up to the lyrics. */
    .np {
      position: fixed;
      z-index: 40;
      overflow: clip;
      touch-action: none;
      --peek: calc(64px + env(safe-area-inset-bottom));
    }
    .art.settling {
      transition: transform 0.12s ease-out, opacity 0.12s ease-out;
    }
    .np:not(.dragging) {
      transition: transform 0.22s cubic-bezier(0.2, 0.8, 0.2, 1);
    }
    /* Everything sits on one 24px gutter: the close button, cover, titles and the icons at
       both ends of the transport row (their buttons pulled out by their padding). */
    /* Header: a bare chevron, the source centered on two lines, the sleep timer. */
    header {
      display: grid;
      grid-template-columns: 64px 1fr 64px;
      padding: calc(4px + env(safe-area-inset-top)) 24px 0;
    }
    header .spacer {
      display: none;
    }
    .round {
      background: none;
      margin-left: -10px;
    }
    .from {
      display: flex;
      flex-direction: column;
      align-items: center;
      text-align: center;
      font-size: 11px;
      letter-spacing: 0.06em;
      text-transform: uppercase;
    }
    .from strong {
      font-size: 13px;
      letter-spacing: 0;
      text-transform: none;
    }
    .desk-only {
      display: none;
    }
    .body {
      overflow: clip;
      padding: 8px 24px calc(var(--peek) + 8px);
      gap: 0;
    }
    /* The cover sits centered in the room above; the song, progress and controls stay
       anchored at the bottom, where the thumb is. */
    /* Fixed, tight steps inside the song block (title, seek bar, times, controls); the spare
       height goes in three equal parts: above the cover, between cover and title, and under
       the controls (the auto margins on .art and .controls). */
    .left {
      flex: 1 1 auto;
      min-height: 0;
      max-width: none;
      justify-content: flex-start;
      gap: 0;
    }
    /* Square at the full width when there's room; shorter phones shrink it (width follows). */
    .art {
      flex: 0 1 calc(100vw - 48px);
      width: auto;
      min-height: 120px;
      margin: auto 0;
      align-self: center;
      box-shadow: 0 16px 36px rgba(0, 0, 0, 0.35);
    }
    /* Spacing like Spotify's player: the cover takes the slack, the bottom group gets a
       steady rhythm (title, 24, seek bar, 12, controls, 24, sheet bar). */
    h1 {
      font-size: 22px;
      line-height: 1.2;
    }
    .artist {
      font-size: 15px;
      margin-top: 2px;
    }
    .album {
      display: none;
    }
    .meta + .progress {
      margin-top: 10px;
    }
    .controls {
      margin-top: 12px;
      margin-bottom: auto;
    }
    .times {
      font-size: 12px;
      margin-top: -4px;
    }
    .meta .heart {
      margin-right: -10px;
    }
    .meta .heart + .heart {
      margin-left: -12px;
    }
    .controls {
      gap: 0;
      justify-content: space-between;
    }
    .phone-only {
      display: flex;
    }
    .side-ctl {
      width: 64px;
      color: var(--np-muted);
      --text: var(--np-text);
      --muted: var(--np-muted);
    }
    .side-ctl.right {
      justify-content: flex-end;
    }
    .side-ctl:not(.right) :global(.icon-btn) {
      margin-left: -11px;
    }
    .side-ctl.right :global(.icon-btn) {
      margin-right: -11px;
    }
    .side-ctl :global(.icon-btn) {
      color: var(--np-muted);
    }
    .side-ctl :global(.icon-btn.on) {
      color: var(--np-text);
    }
    /* The sheet: parked with only its handle and tabs showing, pulled up to just under the
       header. --drag follows a finger. */
    .panel {
      position: absolute;
      left: 0;
      right: 0;
      bottom: 0;
      top: calc(64px + env(safe-area-inset-top));
      height: auto;
      flex: none;
      gap: 10px;
      z-index: 2;
      padding: 0 16px env(safe-area-inset-bottom);
      border-radius: 20px 20px 0 0;
      background: color-mix(in srgb, var(--np-bg-2, var(--np-bg-2-default)) 92%, black);
      box-shadow: 0 -8px 24px rgb(0 0 0 / 0.25);
      transform: translateY(calc(100% - var(--peek) + var(--drag, 0px)));
    }
    .panel.open {
      transform: translateY(var(--drag, 0px));
    }
    /* Parked, only the handle and the tabs show; the list under them stays hidden instead of
       peeking out as a sliver (it fades in as the sheet comes up). */
    /* :global: the lyrics are another component's element, which a scoped selector skipped,
       so their sliver still showed. */
    .panel > :global(:not(.grab):not(.tabs)) {
      transition: opacity 0.2s;
    }
    .panel:not(.open):not(.dragging) > :global(:not(.grab):not(.tabs)) {
      opacity: 0;
      pointer-events: none;
    }
    .panel:not(.dragging) {
      transition: transform 0.28s cubic-bezier(0.2, 0.8, 0.2, 1);
    }
    .grab {
      flex: none;
      height: 20px;
      justify-content: center;
      align-items: center;
    }
    .grab span {
      width: 36px;
      height: 4px;
      border-radius: 2px;
      background: color-mix(in srgb, var(--np-text) 35%, transparent);
    }
    /* Parked, the bar is two even labels; pulled up, the chosen one fills. */
    .tabs {
      flex: none;
      margin-top: -6px;
      display: grid;
      grid-template-columns: 1fr 1fr;
      gap: 8px;
    }
    .tabs .spacer {
      display: none;
    }
    .tabs button {
      width: 100%;
      display: flex;
      align-items: center;
      justify-content: center;
    }
    /* Above Up next's list, so it doesn't scroll away with it. */
    .auto-row {
      background: color-mix(in srgb, var(--np-bg-2, var(--np-bg-2-default)) 92%, black);
      display: flex;
      justify-content: space-between;
      gap: 16px;
      padding: 8px 8px 14px;
      margin-bottom: 4px;
      border-bottom: 1px solid color-mix(in srgb, var(--np-text) 10%, transparent);
      font-size: 14px;
      color: var(--np-text);
    }
    .auto-row > span {
      display: flex;
      flex-direction: column;
      gap: 2px;
    }
    .auto-d {
      font-size: 12px;
      color: var(--np-muted);
    }
    .panel :global(.scroll) {
      touch-action: pan-y;
    }
    .qd {
      display: none;
    }
    .why {
      max-width: 38%;
      overflow: hidden;
      text-overflow: ellipsis;
    }
  }
  :global([data-theme='light']) .panel {
    background: color-mix(in srgb, white 45%, transparent);
  }
  .tabs {
    display: flex;
    align-items: center;
    gap: 6px;
  }
  .tabs button {
    height: 40px;
    padding: 0 18px;
    border-radius: 999px;
    font-weight: 600;
    color: var(--np-muted);
  }
  .tabs button.sel {
    background: var(--np-text);
    color: var(--np-bg-2, var(--np-bg-2-default));
  }
  .auto {
    display: flex;
    align-items: center;
    gap: 8px;
    font-size: 13px;
    color: var(--np-muted);
    cursor: pointer;
  }
  .auto {
    font-weight: 600;
    padding-right: 4px;
  }
  .queue {
    flex: 1;
    min-height: 0;
    display: flex;
    flex-direction: column;
    gap: 2px;
  }
  .qrow {
    display: flex;
    align-items: center;
    gap: 12px;
    padding: 6px 8px;
    border-radius: var(--r-md);
    text-align: left;
    min-height: 52px;
  }
  .qrow:hover {
    background: color-mix(in srgb, var(--np-text) 8%, transparent);
  }
  .qrow {
    transition: background 0.25s, opacity 0.25s;
  }
  /* The song you tapped, playing; the ones you skipped to get there. */
  .qrow.now {
    background: color-mix(in srgb, var(--np-text) 10%, transparent);
  }
  .qrow.now .qt,
  .qrow.now .why {
    color: var(--accent, var(--np-text));
  }
  .qrow.past {
    opacity: 0.45;
  }
  .qart {
    width: 40px;
    height: 40px;
    border-radius: var(--r-sm);
    object-fit: cover;
    flex: none;
    background: color-mix(in srgb, black 25%, transparent);
  }
  .qtext {
    flex: 1;
    min-width: 0;
    display: flex;
    flex-direction: column;
  }
  .qt {
    font-weight: 600;
  }
  .qs,
  .qd,
  .why {
    font-size: 13px;
    color: var(--np-muted);
  }
  .why {
    font-weight: 600;
    white-space: nowrap;
  }
  .qd {
    font-variant-numeric: tabular-nums;
    width: 40px;
    text-align: right;
  }
  .empty {
    color: var(--np-muted);
    padding: 24px;
    font-size: 15px;
  }
  /* After .auto, which would otherwise show both switches everywhere. */
  .auto.phone-only {
    display: none;
  }
  @media (max-width: 640px) {
    .auto.phone-only {
      display: flex;
    }
    .auto.desk-only {
      display: none;
    }
  }
</style>
