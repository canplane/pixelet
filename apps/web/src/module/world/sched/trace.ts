// @pxl/web/module/world/sched/trace.ts

/**
 * Saves the kernel's ledger trace (pxl_core/module/world/sched/trace.rs) as a
 * file, openable at ui.perfetto.dev or chrome://tracing.
 *
 * The kernel builds the JSON and the shell only carries it out of the module's
 * memory. A download rather than the clipboard, and a range rather than a
 * message on the ring: recording covers the whole window, so a trace is
 * routinely tens of megabytes, which the clipboard handles badly or not at all
 * — and Perfetto wants a file anyway.
 *
 * Wired to `window` in dev (ledger_overlay.tsx) so it is reachable from the
 * console. It does not hand the JSON back the way its TypeScript predecessor
 * did: the text now lives in the module's memory, and returning it would mean
 * decoding tens of megabytes into a JavaScript string for a value nothing
 * reads.
 */

import { CORE_KERNEL } from "@/wasm/core_mem.ts";
import { bytes_at, range_at } from "@/wasm/module.ts";

export function ledger_trace_download(): void {
  const at = CORE_KERNEL.ledger_trace();
  const [json_at, bytes] = range_at(CORE_KERNEL.memory, at);
  const mb = (bytes / 1e6).toFixed(1);
  const name = `pxl-ledger-${new Date().toISOString().replace(/[:.]/g, "-")}.json`;
  try {
    // The Blob constructor copies its buffer source, which is what lets the
    // kernel have the bytes back on the next line.
    const url = URL.createObjectURL(new Blob([bytes_at(CORE_KERNEL.memory, json_at, bytes)], { type: "application/json" }));
    CORE_KERNEL.ledger_trace_release();
    const a = document.createElement("a");
    a.href = url;
    a.download = name;
    a.click();
    URL.revokeObjectURL(url);
    console.log(`[ledger] trace saved as ${name} — ${mb}MB. Open it at ui.perfetto.dev`);
  } catch (err) {
    CORE_KERNEL.ledger_trace_release();
    console.log(`[ledger] trace built (${mb}MB) but the download failed:`, err);
  }
}
