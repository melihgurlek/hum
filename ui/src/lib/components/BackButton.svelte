<script lang="ts">
  import { back, ui } from '../state.svelte'
  import Icon from './Icon.svelte'
</script>

{#if ui.history.length}
  <button class="back" aria-label="Back" title="Back (Alt+Left)" onclick={back}><Icon name="back" size={22} /></button>
{/if}

<style>
  .back {
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
  .back:hover {
    background: var(--raised-2);
  }
</style>
