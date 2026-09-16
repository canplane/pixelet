// @pxl/web/module/world/sched/long_frame.ts
//
// The long-frame observers. A frame that blocks the event loop past 50ms is
// a browser measurement of the whole task, and the kernel's tick is only part
// of that task — so it is measured here and handed over, the one number in
// the ledger the kernel cannot take itself.
//
// Both observers feed the same ring and each record says which it came from.
// LoAF is the richer one (a script/render split and the browser's own
// blocking duration) and is Chromium-only; `longtask` reports a duration and
// nothing else but fires everywhere, and a >50ms task is exactly the event
// worth catching. Installed alongside each other rather than instead.
//
// A dev instrument: the shipped monitor cannot lean on LoAF, which is why
// this is wired from the ledger overlay rather than from boot.

import { Cmd } from "@pxl/shared/spec/cmd.ts";

import { CORE_BUILD, packet } from "@/wasm/core_build.ts";

/** As the kernel numbers them (sched/ledger.rs's LongFrameSource). */
const LOAF = 0;
const LONGTASK = 1;

type LoafEntry = PerformanceEntry & {
  blockingDuration?: number;
  renderStart: number;
  scripts?: { duration: number }[];
};

function send(source: number, t: number, duration_ms: number, blocking_ms: number, script_ms: number, render_ms: number): void {
  CORE_BUILD.send(0, Cmd.WORLD_LONG_FRAME, packet((b) => b.u32(source).f64(t).f64(duration_ms).f64(blocking_ms).f64(script_ms).f64(render_ms)));
}

function supports(type: string): boolean {
  return typeof PerformanceObserver !== "undefined" && (PerformanceObserver.supportedEntryTypes?.includes(type) ?? false);
}

/** Whether this browser has long-animation-frame entries at all — Safari and
 * iOS WKWebView do not, and the report says so rather than reading as a
 * window with no long frames in it. */
export function loaf_supported(): boolean {
  return supports("long-animation-frames");
}

let installed = false;

/** Idempotent, and a no-op for an observer this browser lacks. */
export function install_long_frame_observers(): void {
  if (installed) return;
  installed = true;

  if (loaf_supported()) {
    new PerformanceObserver((list) => {
      for (const raw of list.getEntries()) {
        const e = raw as LoafEntry;
        const script_ms = e.scripts?.reduce((sum, s) => sum + s.duration, 0) ?? 0;
        const render_ms = e.renderStart > 0 ? Math.max(0, e.startTime + e.duration - e.renderStart) : 0;
        send(LOAF, e.startTime, e.duration, e.blockingDuration ?? 0, script_ms, render_ms);
      }
    }).observe({ type: "long-animation-frames", buffered: true } as PerformanceObserverInit);
  }

  if (supports("longtask")) {
    new PerformanceObserver((list) => {
      for (const e of list.getEntries()) {
        send(LONGTASK, e.startTime, e.duration, Math.max(0, e.duration - 50), 0, 0);
      }
    }).observe({ type: "longtask", buffered: true } as PerformanceObserverInit);
  }
}
