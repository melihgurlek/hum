<script lang="ts">
  import type { ArtistLink } from '../api'
  import { openArtist, openArtistNamed } from '../state.svelte'

  /** Artist names, each one a link to its page (found by name when we have no id for it). */
  let { artists }: { artists: ArtistLink[] } = $props()
</script>

<!-- {', '} not ", ": Svelte trims the space at the end of a block, which ran duets together. -->
{#each artists as a, i}{#if i > 0}{', '}{/if}<button class="artist-link" onclick={(e) => {
  e.stopPropagation()
  if (a.id) openArtist(a.id)
  else openArtistNamed(a.name)
}}>{a.name}</button>{/each}

<style>
  .artist-link {
    display: inline;
    font: inherit;
    color: inherit;
    padding: 0;
    position: relative;
    z-index: 1;
  }
  /* Hover only where there's a pointer: after a tap a touchscreen keeps :hover, which left
     the underline stuck. */
  @media (hover: hover) {
    .artist-link:hover {
      text-decoration: underline;
    }
  }
  .artist-link:focus-visible {
    text-decoration: underline;
  }
</style>
