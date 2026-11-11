import { svelte } from '@sveltejs/vite-plugin-svelte'
import { defineConfig } from 'vite'

export default defineConfig({
  // Component CSS ships inside each component's JS. Separate virtual CSS modules raced the
  // webview on dev-server restarts ("failed to load virtual css module") and left the app
  // unstyled.
  plugins: [svelte({ emitCss: false })],
  clearScreen: false,
  server: { port: 5173, strictPort: true },
})
