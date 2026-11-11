<script lang="ts">
  import { artistLine, fmtTime, img, type Track } from '../api'
  import { hoverCancel, hoverPrefetch, openAddMenu, player } from '../state.svelte'
  import Icon from './Icon.svelte'
  import type { Snippet } from 'svelte'

  let {
    track,
    onplay,
    detail,
    showDuration = true,
    actions,
  }: {
    track: Track
    onplay: () => void
    detail?: string
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
  <button class="main" onclick={onplay} aria-label={`Play ${track.title} by ${artistLine(track)}`}>
    {#if track.thumb}
      <img class="cover art" src={img(track.thumb)} alt="" loading="lazy" width="44" height="44" />
    {:else}
      <div class="cover art"></div>
    {/if}
    <span class="text">
      <span class="title ellipsis">{track.title}</span>
      <span class="sub ellipsis">{detail ?? artistLine(track)}</span>
    </span>
  </button>
  <span class="tools">
    <button class="tool" aria-label={`Add ${track.title} to a playlist`} title="Add to playlist" onclick={(e) => openAddMenu([track], e)}>
      <Icon name="plus" size={18} />
    </button>
    {@render actions?.()}
  </span>
  {#if showDuration}<span class="dur">{fmtTime(track.duration)}</span>{/if}
</div>

<style>
  .row {
    display: flex;
    align-items: center;
    gap: 4px;
    padding-right: 8px;
    border-radius: 10px;
    min-height: 56px;
  }
  .row:hover,
  .row:focus-within {
    background: var(--raised);
  }
  .main {
    flex: 1;
    min-width: 0;
    display: flex;
    align-items: center;
    gap: 12px;
    padding: 6px 8px;
    text-align: left;
    border-radius: 10px;
  }
  .art {
    width: 44px;
    height: 44px;
    border-radius: 6px;
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
</style>
