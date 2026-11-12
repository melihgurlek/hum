<script lang="ts">
  import { back, ui } from '../state.svelte'
  import Icon from './Icon.svelte'
</script>

<!-- Zero-height and sticky, so the button stays in view while the page scrolls under it. -->
<div class="bar">
  {#if ui.history.length}
    <button class="back" aria-label="Back" title="Back (Alt+Left)" onclick={back}><Icon name="back" size={22} /></button>
  {/if}
</div>

<style>
  .bar {
    position: sticky;
    top: 0;
    height: 0;
    z-index: 5;
    flex: none;
  }
  .back {
    position: absolute;
    top: 16px;
    left: 0;
    box-shadow: 0 2px 10px rgb(0 0 0 / 0.25);
    width: 44px;
    height: 44px;
    border-radius: 50%;
    display: flex;
    align-items: center;
    justify-content: center;
    background: var(--raised);
    color: var(--text);
    flex: none;
  }
  /* Phone: a plain arrow on the gutter, as phone apps draw it, not a raised desktop button.
     A soft shadow keeps it readable over a banner or a scrolled page. When it sticks, it
     stays below the status bar. */
  @media (max-width: 640px) {
    .bar {
      top: env(safe-area-inset-top);
    }
    .back {
      top: -8px;
      margin-left: -11px;
      background: none;
      box-shadow: none;
      filter: drop-shadow(0 1px 3px rgb(0 0 0 / 0.6));
    }
    .back:hover {
      background: none;
    }
  }
  .back:hover {
    background: var(--raised-2);
  }
</style>
