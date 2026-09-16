// @pxl/web/module/world/sched/ledger_overlay.tsx

/**
 * Dev overlay for the kernel's ledger (crates/pxl/core/src/module/world/sched):
 * the recording window and the text side (log.rs) as buttons — a fresh window,
 * the pasteable report, and the live console stream. Gated by
 * `import.meta.env.DEV` and the `show_ledger` setting — off by default.
 *
 * It also owns the dev lifecycle the ledger's surfaces share, which runs
 * whether or not the panel is showing: opening the kernel's recording window,
 * which is off until something asks for it; the long-frame observers, so the
 * ring is already warm when the panel is opened; and `__pxl_ledger_trace` on
 * `window`, so a trace can be taken from the console.
 */

import { useEffect, useState } from "react";

import { ledger_drain_log } from "./ledger_log.ts";
import { install_long_frame_observers } from "./long_frame.ts";
import { ledger_trace_download } from "./trace.ts";
import { kernel_action, useKernel } from "@/wasm/kernel.ts";

/** How often the stream is collected while it is on. */
const DRAIN_MS = 500;

const BUTTON = "pointer-events-auto flex-1 rounded border border-white/15 bg-white/5 py-1 text-white/60 transition-colors hover:bg-white/10 hover:text-white/90";

type LedgerGlobals = typeof globalThis & { __pxl_ledger_trace?: () => void };

export function LedgerOverlay() {
  const show = useKernel((s) => s.settings.show_ledger === true);
  const [logging, set_logging] = useState(false);

  // Nothing is pushed: the kernel keeps the records and this comes for them,
  // in batches rather than per frame — the stream is read while watching, and
  // a drain that ran on every frame would be instrumentation paced by the
  // thing it measures. The buffer is capped, so a slow reader loses the
  // oldest rather than growing the kernel's heap.
  useEffect(() => {
    if (!logging) return;
    const id = setInterval(ledger_drain_log, DRAIN_MS);
    return () => clearInterval(id);
  }, [logging]);

  useEffect(() => {
    if (!import.meta.env.DEV) return;
    // The kernel records nothing until something asks, so that a client with no
    // overlay pays for none of it. This is the asking, and it happens on mount
    // rather than when the panel is opened: what makes a ledger worth having is
    // that the minute before you went looking is already in it.
    kernel_action({ type: "ledger_window_start" });
    // The observers outlive the panel — they are the ledger's, not its, so
    // closing the panel does not stop a window being recorded — but they do
    // not outlive dev. Each long frame they see is a WORLD_LONG_FRAME on the
    // submission ring, and a client with no overlay has nothing to send it to.
    install_long_frame_observers();
    const w = globalThis as LedgerGlobals;
    w.__pxl_ledger_trace = ledger_trace_download;
    return () => {
      delete w.__pxl_ledger_trace;
    };
  }, []);

  if (!import.meta.env.DEV || !show) return null;

  return (
    <div className="pointer-events-none fixed left-2 top-[84px] z-50 w-[228px] rounded-lg bg-black/72 p-2.5 font-mono text-[10.5px] leading-[1.5] text-white/90 backdrop-blur-sm">
      <div className="mb-1 font-semibold tracking-wide text-white/55">sched/ledger</div>
      <div className="mt-1 flex gap-1">
        <button type="button" onClick={() => kernel_action({ type: "ledger_window_start" })} title="clear everything and begin a fresh window" className={BUTTON}>
          start
        </button>
        <button type="button" onClick={() => kernel_action({ type: "ledger_report" })} title="print + copy a text report" className={BUTTON}>
          report
        </button>
        <button type="button" onClick={ledger_trace_download} title="save the window as a Trace Event Format file for ui.perfetto.dev" className={BUTTON}>
          trace
        </button>
        <button
          type="button"
          onClick={() => {
            kernel_action({ type: "ledger_logging", on: !logging });
            set_logging(!logging);
          }}
          title="stream each settled task, missed frame and long frame to the console"
          className={BUTTON}
        >
          {logging ? "log on" : "log"}
        </button>
      </div>
    </div>
  );
}
