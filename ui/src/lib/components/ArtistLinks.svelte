<script lang="ts">
  import type { ArtistLink } from '../api'
  import { openArtist } from '../state.svelte'

  /** Artist names, each one a link to its page when YouTube Music gave it an id. */
  let { artists }: { artists: ArtistLink[] } = $props()
</script>

{#each artists as a, i}{#if i > 0}, {/if}{#if a.id}<button class="artist-link" onclick={(e) => { e.stopPropagation(); openArtist(a.id) }}>{a.name}</button>{:else}{a.name}{/if}{/each}

<style>
  .artist-link {
    display: inline;
    font: inherit;
    color: inherit;
    padding: 0;
    position: relative;
    z-index: 1;
  }
  .artist-link:hover,
  .artist-link:focus-visible {
    text-decoration: underline;
  }
</style>
