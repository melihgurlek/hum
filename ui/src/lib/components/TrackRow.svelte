<script lang="ts">
  import { artistLine, fmtTime, type Track, img } from '../api'
  import { hoverCancel, hoverPrefetch, player } from '../state.svelte'

  let {
    track,
    onplay,
    detail,
    showDuration = true,
  }: { track: Track; onplay: () => void; detail?: string; showDuration?: boolean } = $props()

  const playing = $derived(player.current?.id === track.id)
</script>

<button
  class="row"
  class:playing
  onclick={onplay}
  onmouseenter={() => hoverPrefetch(track.id)}
  onmouseleave={hoverCancel}
  aria-label={`Play ${track.title} by ${artistLine(track)}`}
>
  {#if track.thumb}
    <img class="cover art" src={img(track.thumb)} alt="" loading="lazy" width="44" height="44" />
  {:else}
    <div class="cover art"></div>
  {/if}
  <span class="text">
    <span class="title ellipsis">{track.title}</span>
    <span class="sub ellipsis">{detail ?? artistLine(track)}</span>
  </span>
  {#if showDuration}<span class="dur">{fmtTime(track.duration)}</span>{/if}
</button>

<style>
  .row {
    display: flex;
    align-items: center;
    gap: 12px;
    width: 100%;
    padding: 6px 8px;
    border-radius: 10px;
    text-align: left;
    min-height: 56px;
  }
  .row:hover {
    background: var(--raised);
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
  }
</style>
