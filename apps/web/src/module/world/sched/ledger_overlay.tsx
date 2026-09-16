// @pxl/web/module/world/sched/ledger_overlay.tsx

/**
 * Dev overlay for the kernel's ledger (crates/pxl/core/src/module/world/sched):
 * the recording window and the text side (log.rs) as buttons — a fresh window,
 * the pasteable report, and the live console stream. Gated by
 * `import.meta.env.DEV` and the `show_ledger` setting — off by default.
 *
 * It also owns the dev lifecycle the ledger's surfaces share, which runs
 * whether or not the panel is showing: the long-frame observers, so the ring
 * is already warm when the panel is opened, and `__pxl_ledger_trace` on
 * `window`, so a trace can be taken from the console.
 */

import { useEffect, useState } from "react";

import { install_long_frame_observers } from "./long_frame.ts";
import { ledger_trace_download } from "./trace.ts";
import { kernel_action, useKernel } from "@/wasm/kernel.ts";

const BUTTON = "pointer-events-auto flex-1 rounded border border-white/15 bg-white/5 py-1 text-white/60 transition-colors hover:bg-white/10 hover:text-white/90";

type LedgerGlobals = typeof globalThis & { __pxl_ledger_trace?: () => void };

export function LedgerOverlay() {
  const show = useKernel((s) => s.settings.show_ledger === true);
  // The kernel starts with the stream on (module/world/world.rs), so the
  // button opens saying so rather than sending `on: false` on its first press.
  const [logging, set_logging] = useState(true);

  // The observers outlive the panel: they are the ledger's, not its, so
  // closing the panel does not stop a window being recorded.
  useEffect(install_long_frame_observers, []);

  useEffect(() => {
    if (!import.meta.env.DEV) return;
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
