<script lang="ts">
  import { player } from '../state.svelte'

  /** The "this one is playing" mark: three bars, moving only while audio plays, so it shows
      even when the accent is the plain text color (no cover tint, or a grey cover). */
  let { size = 16 }: { size?: number } = $props()
</script>

<span class="eq" class:moving={!player.paused && !player.loading} style="--s:{size}px" aria-label="Playing" role="img">
  <i></i><i></i><i></i>
</span>

<style>
  .eq {
    display: inline-flex;
    align-items: flex-end;
    justify-content: space-between;
    width: var(--s);
    height: var(--s);
    color: var(--accent, var(--accent-default));
  }
  i {
    width: 22%;
    height: 100%;
    background: currentColor;
    border-radius: 1px;
    transform-origin: bottom;
    transform: scaleY(0.45);
  }
  i:nth-child(2) {
    transform: scaleY(0.8);
  }
  i:nth-child(3) {
    transform: scaleY(0.6);
  }
  /* No animation while paused: nothing repaints when nothing plays. */
  .moving i {
    animation: bar 0.9s ease-in-out infinite alternate;
  }
  .moving i:nth-child(2) {
    animation-duration: 0.7s;
    animation-delay: -0.3s;
  }
  .moving i:nth-child(3) {
    animation-duration: 1.1s;
    animation-delay: -0.6s;
  }
  @keyframes bar {
    from {
      transform: scaleY(0.25);
    }
    to {
      transform: scaleY(1);
    }
  }
  @media (prefers-reduced-motion: reduce) {
    .moving i {
      animation: none;
    }
  }
</style>
