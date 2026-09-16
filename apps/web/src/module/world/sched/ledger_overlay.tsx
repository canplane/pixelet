// @pxl/web/module/world/sched/ledger_overlay.tsx

/**
 * Dev overlay for the kernel's ledger (crates/pxl/core/src/module/world/sched):
 * the recording window and the text side as buttons — a fresh window, the
 * pasteable report, and the live console stream. Gated by
 * `import.meta.env.DEV` and the `show_ledger` setting — off by default.
 *
 * It also installs the long-frame observers, which outlive it: they are the
 * ledger's rather than the panel's, so closing the panel does not stop a
 * window being recorded.
 */

import { useEffect, useState } from "react";

import { install_long_frame_observers } from "./long_frame.ts";
import { kernel_action, useKernel } from "@/wasm/kernel.ts";

const BUTTON = "pointer-events-auto flex-1 rounded border border-white/15 bg-white/5 py-1 text-white/60 transition-colors hover:bg-white/10 hover:text-white/90";

export function LedgerOverlay() {
  const show = useKernel((s) => s.settings.show_ledger === true);
  const [logging, set_logging] = useState(false);

  // The observers outlive the panel: they are the ledger's, not its, so
  // closing the panel does not stop a window being recorded.
  useEffect(install_long_frame_observers, []);

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
