<script lang="ts">
  import { setSleep, sleep } from '../state.svelte'
  import Icon from './Icon.svelte'

  // `size` is the moon's; `align` is which edge of the button the menu lines up with.
  let { size = 18, align = 'right' }: { size?: number; align?: 'left' | 'right' } = $props()

  let open = $state(false)
  // Re-read the clock twice a minute so the minutes left stay current.
  let now = $state(Date.now())
  $effect(() => {
    if (!sleep.until) return
    const t = setInterval(() => (now = Date.now()), 30_000)
    return () => clearInterval(t)
  })
  const label = $derived(
    sleep.endOfSong ? 'End of song' : sleep.until ? `${Math.max(1, Math.ceil((sleep.until - now) / 60_000))} min` : null,
  )
  function pick(c: number | 'song' | null) {
    setSleep(c)
    now = Date.now()
    open = false
  }
</script>

<div class="sleep">
  <button class="icon-btn" class:on={label} aria-label="Sleep timer" aria-expanded={open}
    title={label ? `Sleep timer: ${label}` : 'Sleep timer'} onclick={() => (open = !open)}>
    <Icon name="moon" {size} />{#if label}<span class="left-t">{label}</span>{/if}
  </button>
  {#if open}
    <button class="backdrop" aria-label="Close" onclick={() => (open = false)}></button>
    <div class="menu" class:alignleft={align === 'left'} role="menu" aria-label="Sleep timer">
      <div class="mt">Pause after</div>
      {#each [15, 30, 45, 60] as m}<button role="menuitem" onclick={() => pick(m)}>{m} minutes</button>{/each}
      <button role="menuitem" onclick={() => pick('song')}>End of this song</button>
      {#if label}<button role="menuitem" class="off" onclick={() => pick(null)}>Turn off</button>{/if}
    </div>
  {/if}
</div>

<style>
  .sleep {
    position: relative;
  }
  .sleep .icon-btn {
    width: auto;
    min-width: 36px;
    gap: 6px;
    padding: 0 8px;
  }
  .left-t {
    font-size: 12px;
    font-weight: 700;
    white-space: nowrap;
  }
  .backdrop {
    position: fixed;
    inset: 0;
    z-index: 29;
    cursor: default;
  }
  .menu {
    position: absolute;
    right: 0;
    bottom: calc(100% + 10px);
    z-index: 30;
    width: 200px;
    padding: 6px;
    border-radius: var(--r-lg);
    background: var(--raised);
    border: 1px solid var(--line);
    box-shadow: 0 12px 32px rgb(0 0 0 / 0.35);
    display: flex;
    flex-direction: column;
    color: var(--text);
  }
  .menu.alignleft {
    right: auto;
    left: 0;
  }
  .mt {
    font-size: 12px;
    font-weight: 700;
    color: var(--muted);
    padding: 6px 10px 4px;
  }
  .menu button {
    text-align: left;
    padding: 0 10px;
    min-height: 36px;
    border-radius: var(--r-md);
    font-weight: 600;
  }
  .menu button:hover {
    background: color-mix(in srgb, var(--text) 8%, transparent);
  }
  .menu .off {
    color: var(--muted);
  }
</style>
