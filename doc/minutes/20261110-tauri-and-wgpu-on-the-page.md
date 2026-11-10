# Tauri, and wgpu on the page

**2026-11-10** — recorded with the user. From `ac321a51af`. Whether the app should
leave Expo for Tauri, leave xpute's runtime with it, and run the GPU's records through
wgpu on a page as well. All of it was dropped, none of its code was kept, and it
should not have been run: each reason below stood before anything was built.

## What the user decided

- **Not Tauri.** On a phone it is a system webview, as Capacitor is, and weaker for
  the UI than React Native, which can use native controls. Its host takes Rust alone,
  and the core is to be language-neutral: xpute wraps it so that it takes no such
  constraint.
- **Not wgpu on a page.** WGSL is WebGPU's own language, and symmetry is no reason to
  put wgpu between a page and the API it was written for.
- **xpute stays**, and with it the core's boundary: its memory, `run` and `call`.

## Why the reasons held from the start

- The app's native UI (the iOS panels, glass, SwiftUI controls, the keyboard dock)
  would go. Tauri's advantage is the desktop, which is no target here.
- A Rust host and Rust types across the boundary give up what a C++ core on the same
  runtime shows, and what `AGENTS.md` already held.
- wgpu's `webgpu` backend hands the WGSL over unchanged, and validation and the driver
  are the browser's. It would remove one file, `runtime.ts`, at 1,226 lines, and it
  has no asynchronous pipeline call.
- The stream is already the symmetry: one specification, `spec/gpu/api.json`, and a
  thin runner a platform. On a page it batches the crossing from WebAssembly to
  JavaScript, and on a device it hands the work to another thread.

## What was tried, and what it was worth

A wgpu surface under a transparent Tauri webview composed on macOS, which says nothing
of a phone: no supported way was found to place a native view beside its webview.
`xpute-gpu`'s Rust runner ran inside the page's module on wgpu's `webgpu` backend.

Measuring could not have decided this, and did not:

| | runner in TypeScript | runner on wgpu | worth |
| --- | --- | --- | --- |
| Desktop Chrome, fps | 44.4, 44.7 | 45.6, 45.4 | equal |
| S23 Chrome, fps | 10.5, 8.0 | 13.5, 12.9, 14.8 | no temperature read; every pass fell alike, unexplained |
| Mac Safari, GPU p50, ms | 16.5, 35.3, 21.8, 43.2 | 21.5, 21.0 | the TypeScript runner differs from itself by 2.61x |
| iPhone 13 mini | one run | none | no comparison |
| `core.wasm`, gzip | 787,246 | 985,354 | a cost |

An agent's statement during the session that wgpu doubled Safari's GPU p95 did not
reproduce, and part of those runs shared Safari with another session.
