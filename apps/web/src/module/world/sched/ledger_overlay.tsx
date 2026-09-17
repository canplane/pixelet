// @pxl/web/module/world/sched/ledger_overlay.tsx

/**
 * Dev overlay for the kernel's ledger (crates/pxl/core/src/module/world/sched):
 * the recording window and the exports as buttons, over the gauge panel
 * (ledger_gauge.tsx). Gated by `import.meta.env.DEV` and the `show_ledger`
 * setting — off by default.
 *
 * It also owns the dev lifecycle the ledger's surfaces share, which runs
 * whether or not the panel is showing: opening the kernel's recording window,
 * which is off until something asks for it; the long-frame observers, so the
 * ring is already warm when the panel is opened; and `__pxl_ledger_trace` on
 * `window`, so a trace can be taken from the console.
 */

import { useEffect, useState } from "react";

import { LedgerGauge } from "./ledger_gauge.tsx";
import { ledger_drain_log, ledger_log_long_frames } from "./ledger_log.ts";
import { ledger_report_print, ledger_report_text } from "./ledger_report.ts";
import { ledger_report_read } from "./ledger_view.ts";
import { install_long_frame_observers, long_frames_clear } from "./long_frame.ts";
import { ledger_trace_download } from "./trace.ts";
import { kernel_action, useKernel } from "@/wasm/kernel.ts";

/** How often the stream is collected while it is on. */
const DRAIN_MS = 500;

const BUTTON = "pointer-events-auto flex-1 rounded border border-white/15 bg-white/5 py-1 text-white/60 transition-colors hover:bg-white/10 hover:text-white/90";

type LedgerGlobals = typeof globalThis & { __pxl_ledger_trace?: () => void };

/** A fresh window on both sides: the kernel's rings and the shell's long frames. */
function start_window(): void {
  kernel_action({ type: "ledger_window_start" });
  long_frames_clear();
}

export function LedgerOverlay() {
  const show = useKernel((s) => s.settings.show_ledger === true);
  const [logging, set_logging] = useState(false);

  // Nothing is pushed: the kernel keeps the rows and this comes for them,
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
    start_window();
    // The observers outlive the panel — they are the ledger's, not its, so
    // closing the panel does not stop a window being recorded — but they do
    // not outlive dev: a client with no overlay has no one to read what they
    // keep.
    install_long_frame_observers();
    const w = globalThis as LedgerGlobals;
    w.__pxl_ledger_trace = ledger_trace_download;
    return () => {
      delete w.__pxl_ledger_trace;
    };
  }, []);

  if (!import.meta.env.DEV || !show) return null;

  return (
    <div className="pointer-events-none fixed left-2 top-[84px] z-50 max-h-[calc(100dvh-96px)] w-[228px] overflow-hidden rounded-lg bg-black/72 p-2.5 font-mono text-[10.5px] leading-[1.5] text-white/90 backdrop-blur-sm">
      <div className="mb-1 font-semibold tracking-wide text-white/55">sched/ledger, last 10s</div>

      {
        /* The buttons come before the numbers: on a phone the panel runs past
          the bottom of the screen, and what must stay reachable is start/stop
          and the exports. The body below is clipped rather than scrolled — the
          panel lets gestures through to the map, and a scrolling body would
          not. */
      }
      <div className="mt-1 flex gap-1">
        {
          /* start and stop are both present rather than one toggle: the dev
            overlay opens a window on mount so a cold-load stall is never
            missed, so a toggle would read "stop" until you had stopped once —
            and then "start" is the button you cannot find when you want a
            clean window. */
        }
        <button type="button" onClick={start_window} title="clear everything and begin a fresh window" className={BUTTON}>
          start
        </button>
        <button type="button" onClick={() => kernel_action({ type: "ledger_window_stop" })} title="freeze the window so exports are stable while you read them" className={BUTTON}>
          stop
        </button>
      </div>
      <div className="mt-1 flex gap-1">
        <button type="button" onClick={() => void ledger_report_print(ledger_report_text(ledger_report_read()))} title="print + copy a text report" className={BUTTON}>
          report
        </button>
        <button type="button" onClick={ledger_trace_download} title="save the window as a Trace Event Format file for ui.perfetto.dev" className={BUTTON}>
          trace
        </button>
        <button
          type="button"
          onClick={() => {
            kernel_action({ type: "ledger_logging", on: !logging });
            ledger_log_long_frames(!logging);
            set_logging(!logging);
          }}
          title="stream each settled task, missed frame and long frame to the console"
          className={BUTTON}
        >
          {logging ? "log on" : "log"}
        </button>
      </div>
      <div className="my-1 border-t border-white/10" />

      <LedgerGauge />
    </div>
  );
}
