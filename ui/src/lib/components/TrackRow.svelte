<script lang="ts">
  import { artistLine, artistLinks, fmtTime, img, type Track } from '../api'
  import { hoverCancel, hoverPrefetch, layout, openAddMenu, player } from '../state.svelte'
  import Icon from './Icon.svelte'
  import ArtistLinks from './ArtistLinks.svelte'
  import Playing from './Playing.svelte'
  import type { Snippet } from 'svelte'

  let {
    track,
    onplay,
    detail,
    suffix,
    number,
    showDuration = true,
    actions,
  }: {
    track: Track
    onplay: () => void
    /** Replaces the artist line. */
    detail?: string
    /** Shown after the (linked) artist names, e.g. ", 2 days ago". */
    suffix?: string
    /** Show this track number instead of the cover (album pages). */
    number?: number
    showDuration?: boolean
    /** Extra buttons shown on hover, after "Add to playlist" (e.g. Remove). */
    actions?: Snippet
  } = $props()

  const playing = $derived(player.current?.id === track.id)
</script>

<div
  class="row"
  class:playing
  role="group"
  aria-label={`${track.title} by ${artistLine(track)}`}
  onmouseenter={() => hoverPrefetch(track.id)}
  onmouseleave={hoverCancel}
  oncontextmenu={(e) => openAddMenu([track], e)}
>
  <!-- The play button stretches over the whole row (see .main::after); artist links sit above it. -->
  {#if number != null}
    <span class="num">{#if playing}<Playing />{:else}<span aria-hidden="true">{number}</span>{/if}</span>
  {:else}
    <span class="artwrap">
      {#if track.thumb}
        <img class="cover art" src={img(track.thumb)} alt="" loading="lazy" width="44" height="44" />
      {:else}
        <div class="cover art"></div>
      {/if}
      {#if playing}<span class="eqover"><Playing /></span>{/if}
    </span>
  {/if}
  <span class="text">
    <button class="main title ellipsis" onclick={onplay} aria-label={`Play ${track.title} by ${artistLine(track)}`}>{track.title}</button>
    {#if detail !== ''}<span class="sub ellipsis">{#if detail != null}{detail}{:else if layout.phone}{artistLine(track)}{suffix ?? ''}{:else}<ArtistLinks artists={artistLinks(track)} />{suffix ?? ''}{/if}</span>{/if}
  </span>
  <span class="tools">
    <!-- Phone: ⋮ for the song's menu (also on a long press); its artist links live there, not
         in the row, where they were easy to hit when you meant to play. -->
    <button class="tool" aria-label={layout.phone ? `More for ${track.title}` : `Add ${track.title} to a playlist`}
      title={layout.phone ? 'More' : 'Add to playlist'} onclick={(e) => openAddMenu([track], e)}>
      <Icon name={layout.phone ? 'more' : 'plus'} size={18} />
    </button>
    {@render actions?.()}
  </span>
  {#if showDuration}<span class="dur">{fmtTime(track.duration)}</span>{/if}
</div>

<style>
  .row {
    position: relative;
    display: flex;
    align-items: center;
    gap: 12px;
    padding: 6px 16px 6px 8px;
    border-radius: var(--r-md);
    min-height: 56px;
    /* Off-screen rows in long lists (liked songs, history) skip layout and paint. */
    content-visibility: auto;
    contain-intrinsic-size: auto 56px;
  }
  .row:hover,
  .row:focus-within {
    background: var(--raised);
  }
  .main {
    display: block;
    max-width: 100%;
    padding: 0;
    text-align: left;
    font: inherit;
    color: inherit;
  }
  .main::after {
    content: '';
    position: absolute;
    inset: 0;
    border-radius: var(--r-md);
  }
  .main:focus-visible {
    outline: none;
  }
  .main:focus-visible::after {
    outline: 2px solid var(--accent, var(--accent-default));
    outline-offset: -2px;
  }
  .artwrap {
    position: relative;
    flex: none;
  }
  .eqover {
    position: absolute;
    inset: 0;
    display: flex;
    align-items: center;
    justify-content: center;
    border-radius: var(--r-sm);
    background: rgb(0 0 0 / 0.55);
  }
  /* On the dark scrim the bars read best in white, whatever the accent. */
  .eqover :global(.eq) {
    color: #fff;
  }
  .num {
    width: 44px;
    flex: none;
    display: flex;
    justify-content: center;
    color: var(--muted);
    font-variant-numeric: tabular-nums;
  }
  .playing .num {
    color: var(--accent, var(--accent-default));
    font-weight: 700;
  }
  .art {
    width: 44px;
    height: 44px;
    border-radius: var(--r-sm);
    flex: none;
  }
  .text {
    display: flex;
    flex-direction: column;
    min-width: 0;
    flex: 1;
  }
  .title {
    font-weight: 600;
  }
  .playing .title {
    color: var(--accent, var(--accent-default));
  }
  .sub,
  .dur {
    color: var(--muted);
    font-size: 13px;
  }
  .dur {
    font-variant-numeric: tabular-nums;
    min-width: 36px;
    text-align: right;
  }
  .tools {
    display: flex;
    opacity: 0;
    position: relative;
    z-index: 1;
  }
  .row:hover .tools,
  .row:focus-within .tools {
    opacity: 1;
  }
  .tools :global(.tool) {
    width: 36px;
    height: 36px;
    border-radius: 50%;
    display: flex;
    align-items: center;
    justify-content: center;
    color: var(--muted);
  }
  .tools :global(.tool:hover) {
    color: var(--text);
    background: var(--raised-2);
  }
  /* Touch screens have no hover: keep the row's tools in reach. */
  /* Phone: covers and durations line up with the page's 16px gutter, not 8px inside it. */
  @media (max-width: 640px) {
    .row {
      padding: 6px 8px;
      margin: 0 -8px;
    }
  }
  @media (hover: none) {
    .tools {
      opacity: 1;
    }
  }
</style>
