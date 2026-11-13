# hum: notes for coding agents

## Desktop and Android share one UI

`ui/` serves both: desktop (Linux, Windows) and the phone (`layout.phone`, `@media (max-width: 640px)`).
`crates/core` is shared too; `src-tauri` has desktop and Android paths (`#[cfg(target_os = "android")]`).

Every fix or improvement on one platform gets checked on the other in the same task:
- Does the other platform have the same bug, or need the same change? Fix it there too.
- Did the change alter the other platform without meaning to (a shared component, a default,
  a CSS rule outside a media query)? Look at it there before calling it done.
- Say in the PR what was checked on each platform, and what wasn't.

Desktop UI can be checked in Chrome (vite on :5174 with a stubbed Tauri). The phone layout needs
a 412x891 viewport or the phone itself (`adb exec-out screencap -p`); safe-area insets only show
on the phone.
