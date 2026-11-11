<script lang="ts">
  /** The "this one is playing" mark: three bars of different heights, so it shows even when the
      accent is the plain text color (no cover tint, or a grey cover). Static on purpose: any
      endless CSS animation keeps the webview redrawing every frame (~16 % of a core). */
  let { size = 16 }: { size?: number } = $props()
</script>

<span class="eq" style="--s:{size}px" aria-label="Playing" role="img">
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
</style>
