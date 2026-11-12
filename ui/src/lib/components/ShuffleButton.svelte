<script lang="ts">
  import { setShuffle, ui } from '../state.svelte'
  import Icon from './Icon.svelte'

  let { size = 20 }: { size?: number } = $props()
</script>

<button class="icon-btn" class:on={ui.shuffle} aria-pressed={ui.shuffle} aria-label="Shuffle"
  title={ui.shuffle ? 'Shuffle: on' : 'Shuffle: off'} onclick={() => setShuffle(!ui.shuffle)}>
  <Icon name="shuffle" {size} />
</button>
