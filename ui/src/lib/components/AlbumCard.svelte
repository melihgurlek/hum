<script lang="ts">
  import { img, type AlbumCard } from '../api'
  import { openAlbum, playAlbum } from '../state.svelte'
  import Icon from './Icon.svelte'

  // A playlist card passes its own `subtitle` and actions, since it has no page in hum.
  let {
    album,
    showArtist = false,
    subtitle,
    onplay,
    onopen,
  }: { album: AlbumCard; showArtist?: boolean; subtitle?: string | null; onplay?: () => void; onopen?: () => void } = $props()
  const meta = $derived(
    subtitle !== undefined
      ? subtitle
      : [album.year, album.kind === 'Album' && !showArtist ? null : album.kind, showArtist ? album.artists.map((a) => a.name).join(', ') : null]
          .filter(Boolean)
          .join(' · '),
  )
</script>

<div class="card">
  <div class="artwrap">
    {#if album.thumb}<img class="cover art" src={img(album.thumb)} alt="" loading="lazy" />{:else}<div class="cover art"></div>{/if}
    <button class="play" aria-label={`Play ${album.title}`} title="Play" onclick={() => (onplay ? onplay() : playAlbum(album.id))}>
      <Icon name="play" size={18} />
    </button>
  </div>
  <button class="open ellipsis" onclick={() => (onopen ? onopen() : openAlbum(album.id))}>{album.title}</button>
  {#if meta}<span class="meta ellipsis">{meta}</span>{/if}
</div>

<style>
  .card {
    position: relative;
    display: flex;
    flex-direction: column;
    gap: 4px;
    min-width: 0;
    padding: 8px;
    margin: -8px;
    border-radius: var(--r-lg);
  }
  .card:hover {
    background: var(--raised);
  }
  .artwrap {
    position: relative;
    margin-bottom: 4px;
  }
  .art {
    width: 100%;
    aspect-ratio: 1;
    border-radius: var(--r-md);
    display: block;
  }
  /* The title button covers the whole card; the play button sits above it. */
  .open {
    padding: 0;
    text-align: left;
    font: inherit;
    font-weight: 600;
    color: inherit;
  }
  .open::after {
    content: '';
    position: absolute;
    inset: 0;
    border-radius: var(--r-lg);
  }
  .open:focus-visible {
    outline: none;
  }
  .open:focus-visible::after {
    outline: 2px solid var(--accent, var(--accent-default));
  }
  .meta {
    color: var(--muted);
    font-size: 13px;
  }
  .play {
    position: absolute;
    right: 8px;
    bottom: 8px;
    z-index: 1;
    width: 44px;
    height: 44px;
    border-radius: 50%;
    display: flex;
    align-items: center;
    justify-content: center;
    background: var(--accent, var(--accent-default));
    color: var(--on-accent);
    box-shadow: 0 4px 14px rgb(0 0 0 / 0.35);
    opacity: 0;
    transform: translateY(4px);
    transition: opacity 0.15s, transform 0.15s;
  }
  .card:hover .play,
  .play:focus-visible {
    opacity: 1;
    transform: none;
  }
  @media (hover: none) {
    .open:active {
      transform: none;
      opacity: 1;
    }
    .open::after {
      transition: background-color 0.12s ease-out;
    }
    .open:active::after {
      background: color-mix(in srgb, var(--text) 6%, transparent);
      transition: none;
    }
  }
</style>
