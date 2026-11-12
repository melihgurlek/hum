<script lang="ts">
  import { setRepeat, ui } from '../state.svelte'
  import Icon from './Icon.svelte'

  let { size = 20 }: { size?: number } = $props()
  const next = { off: 'all', all: 'one', one: 'off' } as const
  const label = { off: 'Repeat: off', all: 'Repeat: the whole queue', one: 'Repeat: this song' }
</script>

<button class="icon-btn repeat" class:on={ui.repeat !== 'off'} aria-label={label[ui.repeat]} title={label[ui.repeat]}
  onclick={() => setRepeat(next[ui.repeat])}>
  <Icon name="repeat" {size} />
  {#if ui.repeat === 'one'}<span class="one" aria-hidden="true">1</span>{/if}
</button>

<style>
  .repeat {
    position: relative;
  }
  /* Repeat one: a small 1 on the arrows, the usual sign. */
  .one {
    position: absolute;
    top: 6px;
    right: 5px;
    width: 13px;
    height: 13px;
    border-radius: 50%;
    font-size: 9px;
    font-weight: 800;
    line-height: 13px;
    text-align: center;
    background: var(--accent, var(--accent-default));
    color: var(--on-accent);
  }
</style>
