# Concurrency Model: why preemptive threading over SAB was abandoned

**2026-07-08**, with confirmations on 2026-08-10 and 2026-08-11 — why the
WASM + SharedArrayBuffer + preemptive-threading + offscreen-WebGPU direction was
stopped, and what shape a restart should take instead.

**Status: parked.** Not rejected on principle — re-evaluated when the project
leaves the MVP milestone.

---

## What was attempted

Optimizing `pxl/render`'s offscreen WebGPU path to its limit by putting Rust
threads on web workers, tying them together with SharedArrayBuffer, and running
them preemptively the way OS threads work. In effect: reproducing a native
threading model inside a browser tab.

## Why it stopped

**The complexity exceeded the benefit.** Three specific things, and the third is
the one that actually broke it.

**1. `wgpu` was not as mature as Three.js.** Rendering WebGPU from inside Rust
meant worse engine and library tooling than what Three.js already provided, and
offscreen rendering through OffscreenCanvas was not officially supported at the
time.

**2. The SAB dilemma had no good side.** Using SharedArrayBuffer forces
COOP/COEP cross-origin isolation on the _entire application_. Not using it means
paying a structured-copy cost every time data crosses a worker or WASM boundary.
Both options were bad, and the choice looked forced.

**3. The premise turned out to be false.** Figma — the reference point for
"serious canvas application on the web" — **does not use SharedArrayBuffer.** It
runs what is effectively a single-threaded environment with cooperative
multitasking. Once that was known, the whole "we need SAB and preemption"
premise collapsed. There was no example of the thing being imitated.

Underneath all three: **browser workers are event-loop based, so real preemption
does not exist there anyway.** And WebGPU command submission is already
asynchronous and queued, so OS-thread-grade parallelism was never the
prerequisite it was assumed to be.

## A third option: transferable objects

"SAB or copy overhead" was never the real choice. **Transferable objects** are
the third option: `postMessage` with a transfer list moves `ArrayBuffer`
ownership zero-copy. Not shared memory, so no COOP/COEP; not structured clone,
so no copy.

This matters specifically here because **XTP is already an ArrayBuffer-backed
binary tree** (`packages/xpute/core/src/io/marshal/tree/`) — it fits the transfer
pattern directly, with no adaptation.

## What a restart should look like

Not a return to the abandoned design. The shape agreed on, assuming a Capacitor
mobile target:

- **Concurrency:** main thread (UI + render, rAF loop) plus a small fixed number
  of long-lived workers — one for world/terrain, say. Cooperative and
  message-passing, never preemptive-with-SAB.
- **Transport between them:** transferable objects, per above.
- **WASM scope:** narrow numeric kernels inside a worker (terrain baking, sparse
  tree operations) — not the whole core. Single-threaded WASM modules, no
  wasm-threads, no SAB-backed memory.
- **Rendering:** Three.js/WebGL as the baseline target.

**Why the Capacitor constraint shapes this:** Capacitor is a plain WebView
wrapper (iOS WKWebView / Android System WebView), not Electron. SAB and COOP
support varies by OS and WebView version, and old Android System WebView is not
trustworthy for it. Web Workers plus single-threaded WASM are reliably supported
on both.

**Update 2026-08-10 — WebGPU as a hard dependency:** the "WebGPU is progressive
enhancement only" caveat above assumed support for older devices. With the
support floor set explicitly at 2023-and-later devices on near-current OSes,
WebGPU is stable enough to depend on — iOS enables it in Safari/WKWebView from
iOS 18, and Android's System WebView updates through the Play Store independently
of the OS. **But this changes only feasibility, not scope**: the engineering-effort
argument below is unaffected by device support.

## Re-confirmed 2026-08-10: this is dead for now, and the reason is scale

Reintroducing a WASM worker is not "adding one WASM module". It pulls in ripping
out the engine and re-attaching WebGL/WebGPU, cooperative scheduling, data
copying, and offscreen-render issues — an engine-level rewrite on the scale of
what Figma or eaglercraft did. That scale does not fit the MVP stage.

**Consequence for the infrastructure built during the WASM era:** XTP and
`sparse_tree/` are deliberately retained as insurance against a conditional
restart, even though nothing uses them right now. Their quality was assessed
and found good — relocatable offsets, WORD alignment, typed-optional
semantics, zero-copy cursor reads. Until a restart condition actually holds,
neither trim them nor invest further in them.

## Restart conditions and cautions

The trigger is **the transition out of the MVP phase into the next milestone** —
not a vague "if this becomes a real business". The deferred RPC/P2P direction
gets re-evaluated at the same moment; treat the two as one bundle.

Two cautions recorded for that point:

**Verify VS Code's actual model before copying it.** VS Code is a restart
reference candidate because it uses workers and SAB in varied ways — but VS Code
desktop's multi-process architecture (extension host and friends) depends on
**real OS processes provided by Electron**, which a browser tab does not have.
Whether the browser build (vscode.dev) is actually SAB-backed preemptive or
Figma-style cooperative message-passing needs checking _first_. Assuming
otherwise repeats exactly the mistake made with Skype and Figma: pattern-matching
on a reference whose real constraints turned out not to apply.

**Start from a blank project and move files in one at a time**, rather than
reshaping this repository in place — porting only the assets that have proven
themselves (the baking pipeline, XTP, sparse tree, floating-origin anchors, the
shared kit).

## Related

The retrospective on the folder structure this era produced — and the observation
that the most design energy went into the layer that needed generalization least
— is in [20260708-vision-and-scope-pivot.md](./20260708-vision-and-scope-pivot.md).

Figma's rendering architecture, looked at directly in 2026-08, is covered in
[20260816-csr-vs-ssr.md](./20260816-csr-vs-ssr.md#prior-art-figma) — it confirms
the cooperative-multitasking reading that broke the premise here.

## Status 2026-09-13

The restart trigger arrived in a different form than expected. The WASM
direction is decided again
([20260912-why-the-destination-is-wasm.md](./20260912-why-the-destination-is-wasm.md))
— without SAB or preemptive threads, on one cooperative thread, which is
what "what a restart should look like" above asked for. The "start from a
blank project" caution does not carry: the state is being moved into the
shape the core wants inside this repository, one lane set at a time, so
the port begins from a client whose memory is already linear rather than
from a checkpoint.
