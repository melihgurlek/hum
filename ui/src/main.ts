import { mount } from 'svelte'
import './app.css'
import App from './App.svelte'
import { forwardErrors } from './lib/debug'
import { ripples } from './lib/ripple'

forwardErrors()
ripples()

const app = mount(App, {
  target: document.getElementById('app')!,
})

export default app
