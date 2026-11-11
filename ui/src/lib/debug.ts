import { invoke } from '@tauri-apps/api/core'

/** Forward webview errors and failed resource loads to the Rust log. */
export function forwardErrors() {
  const send = (message: string) => invoke('ui_log', { message }).catch(() => {})
  window.addEventListener(
    'error',
    (e) => {
      const t = e.target as HTMLElement | null
      if (t && t !== (window as unknown) && 'src' in t) send(`failed to load ${t.tagName} ${(t as HTMLImageElement).src}`)
      else send(`error: ${e.message} at ${e.filename}:${e.lineno}`)
    },
    true,
  )
  window.addEventListener('unhandledrejection', (e) => send(`unhandled rejection: ${String(e.reason)}`))
}
