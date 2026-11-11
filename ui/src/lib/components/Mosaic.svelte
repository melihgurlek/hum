<script lang="ts">
  import { img } from '../api'
  let { covers, size = 48, radius = 6 }: { covers: string[]; size?: number; radius?: number } = $props()
</script>

<div class="mosaic" class:grid={covers.length >= 4} style="width:{size}px;height:{size}px;border-radius:{radius}px">
  {#if covers.length >= 4}
    {#each covers.slice(0, 4) as c}<img src={img(c)} alt="" loading="lazy" />{/each}
  {:else if covers.length}
    <img src={img(covers[0])} alt="" loading="lazy" />
  {:else}
    <svg class="empty" width={size * 0.4} height={size * 0.4} viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" aria-hidden="true"><path d="M9 18V5l12-2v13" /><circle cx="6" cy="18" r="3" /><circle cx="18" cy="16" r="3" /></svg>
  {/if}
</div>

<style>
  .mosaic {
    overflow: hidden;
    background: var(--raised-2);
    flex: none;
    display: flex;
    align-items: center;
    justify-content: center;
    color: var(--muted);
  }
  .mosaic.grid {
    display: grid;
    grid-template-columns: 1fr 1fr;
  }
  img {
    width: 100%;
    height: 100%;
    object-fit: cover;
    display: block;
  }
</style>
