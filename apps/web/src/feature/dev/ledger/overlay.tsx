// @pxl/web/feature/dev/ledger/overlay.tsx

/**
 * Dev overlay for the kernel's ledger (crates/pxl/kernel/src/sched/ledger):
 * the recording window and the exports as buttons, over the gauge panel
 * (gauge.tsx). Gated by `import.meta.env.DEV` and the `show_ledger`
 * setting — off by default.
 *
 * It also owns the dev lifecycle the ledger's surfaces share, which runs
 * whether or not the panel is showing: opening the kernel's recording window,
 * which is off until something asks for it; the long-frame observers, so the
 * ring is already warm when the panel is opened; and the handles on `window`
 * an A/B is driven and read through from the console — `__pxl_ledger_trace`,
 * `__pxl_view` and `__pxl_gauge`. Two builds are only comparable read at
 * the same view, which is what `__pxl_view` puts the camera at (INPUT_VIEW),
 * and the numbers are the panel's own.
 */

import { useEffect, useState } from "react";

import { LedgerGauge } from "./gauge.tsx";
import { ledgerDrainLog, ledgerLogLongFrames } from "./log.ts";
import { ledgerReportPrint, ledgerReportText } from "./report.ts";
import { type Gauge, ledgerGaugeRead, ledgerReportRead } from "./view.ts";
import { installLongFrameObservers, longFramesClear } from "./long-frame.ts";
import { ledgerTraceDownload } from "./trace.ts";
import { Command, encode_input_view, encode_ledger_log } from "@/kernel/abi/cmd.spec.ts";
import { kernel_command, useKernel } from "@/kernel/shell.ts";
import { SETTINGS } from "@/feature/setting/schema.ts";

/** How often the stream is collected while it is on. */
const DRAIN_MS = 500;

const BUTTON = "pointer-events-auto flex-1 rounded border border-white/15 bg-white/5 py-1 text-white/60 transition-colors hover:bg-white/10 hover:text-white/90";
/** start, when a window is stopped and one can begin; stop, while one is open. */
const BUTTON_GO = "pointer-events-auto flex-1 rounded border border-emerald-400/40 bg-emerald-400/10 py-1 text-emerald-300 transition-colors hover:bg-emerald-400/20";
const BUTTON_HALT = "pointer-events-auto flex-1 rounded border border-amber-400/40 bg-amber-400/10 py-1 text-amber-300 transition-colors hover:bg-amber-400/20";
const BUTTON_OFF = "pointer-events-auto flex-1 rounded border border-white/10 bg-white/5 py-1 text-white/25";

/** How often the panel repaints. */
const POLL_MS = 500;

/** The panel reads the recent past, not the whole recording. Folding every
 * frame since load on each repaint grew into a 60 ms task every half second
 * two minutes in — the panel was the stall it was showing. The report still
 * covers the whole window. */
const LIVE_WINDOW_MS = 10_000;

type LedgerGlobals = typeof globalThis & {
  __pxl_ledger_trace?: () => void;
  __pxl_view?: (dist: number | null, polar: number | null) => void;
  __pxl_gauge?: (window_ms: number) => Gauge;
};

/** A fresh window on both sides: the kernel's rings and the shell's long frames. */
function startWindow(): void {
  kernel_command(Command.LEDGER_START);
  longFramesClear();
}

export function LedgerOverlay() {
  const show = useKernel((s) => import.meta.env.DEV && s.settings[SETTINGS.show_ledger.key] === 1);
  const [logging, setLogging] = useState(false);
  const [g, setG] = useState<Gauge | null>(null);

  useEffect(() => {
    if (!show) return;
    const read = () => setG(ledgerGaugeRead(LIVE_WINDOW_MS));
    read();
    const id = setInterval(read, POLL_MS);
    return () => clearInterval(id);
  }, [show]);

  // Nothing is pushed: the kernel keeps the rows and this comes for them,
  // in batches rather than per frame — the stream is read while watching, and
  // a drain that ran on every frame would be instrumentation paced by the
  // thing it measures. The buffer is capped, so a slow reader loses the
  // oldest rather than growing the kernel's heap.
  useEffect(() => {
    if (!logging) return;
    const id = setInterval(ledgerDrainLog, DRAIN_MS);
    return () => clearInterval(id);
  }, [logging]);

  useEffect(() => {
    if (!import.meta.env.DEV) return;
    // The kernel records nothing until something asks, so that a client with no
    // overlay pays for none of it. This is the asking, and it happens on mount
    // rather than when the panel is opened: what makes a ledger worth having is
    // that the minute before you went looking is already in it.
    startWindow();
    // The observers outlive the panel — they are the ledger's, not its, so
    // closing the panel does not stop a window being recorded — but they do
    // not outlive dev: a client with no overlay has no one to read what they
    // keep.
    installLongFrameObservers();
    const w = globalThis as LedgerGlobals;
    w.__pxl_ledger_trace = ledgerTraceDownload;
    w.__pxl_view = (dist, polar) => kernel_command(Command.INPUT_VIEW, encode_input_view({ dist, polar }));
    w.__pxl_gauge = (window_ms) => ledgerGaugeRead(window_ms);
    return () => {
      delete w.__pxl_ledger_trace;
      delete w.__pxl_view;
      delete w.__pxl_gauge;
    };
  }, []);

  if (!import.meta.env.DEV || !show) return null;
  const recording = g?.rec.recording === true;

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
        <button type="button" onClick={startWindow} title="clear everything and begin a fresh window" className={recording ? BUTTON : BUTTON_GO}>
          start
        </button>
        <button
          type="button"
          onClick={() => kernel_command(Command.LEDGER_STOP)}
          disabled={!recording}
          title="freeze the window so exports are stable while you read them"
          className={recording ? BUTTON_HALT : BUTTON_OFF}
        >
          stop
        </button>
      </div>
      <div className="mt-1 flex gap-1">
        <button type="button" onClick={() => void ledgerReportPrint(ledgerReportText(ledgerReportRead()))} title="print + copy a text report" className={BUTTON}>
          report
        </button>
        <button type="button" onClick={ledgerTraceDownload} title="save the window as a Trace Event Format file for ui.perfetto.dev" className={BUTTON}>
          trace
        </button>
        <button
          type="button"
          onClick={() => {
            kernel_command(Command.LEDGER_LOG, encode_ledger_log({ on: !logging }));
            ledgerLogLongFrames(!logging);
            setLogging(!logging);
          }}
          title="stream each settled task, missed frame and long frame to the console"
          className={BUTTON}
        >
          {logging ? "log on" : "log"}
        </button>
      </div>
      <div className="my-1 border-t border-white/10" />

      <LedgerGauge g={g} />
    </div>
  );
}
