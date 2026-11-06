// @pxl/client/runtime/start.ts

/**
 * xpute's runtime mounted on the kernel where the kernel's memory is: the
 * kernel's module, the files (sys/), the stop report, the GPU, the frame pacing
 * and each turn's grant (port/surface.ts). A device runs it on the shell's
 * JavaScript thread (service/runtime.native.ts); a page in the kernel's worker
 * (runtime/worker.ts). What the kernel posts goes to `program`.
 */

import { boot as boot_runtime, FramePacing } from "@xpute/runtime";
import type { Abort, OnImport, Runtime } from "@xpute/runtime";
import { MACHINE } from "@/abi/machine.spec.ts";
import {
  BATCH_FRAMES,
  MARGIN_SHARE,
  PACE_HZ,
  SETTLE_MS,
} from "@/abi/pacing.spec.ts";
import { namespace } from "@/sys/fs/namespace.ts";
import { submit_grant } from "@/port/surface.ts";
import * as device from "@/service/device.ts";
import { source } from "@/service/kernel.ts";
import { gpu } from "@/service/gpu.ts";

/** The runtime booted; `onabort` hears the kernel's stop report and
 * `onterminated` that it stopped. */
export async function start(
  program: OnImport,
  onabort: (report: Abort) => void,
  onterminated: () => void,
): Promise<{ runtime: Runtime; pacing: FramePacing }> {
  // A phone gets less: JavaScriptCore counts the whole reservation as page memory.
  const sizes = MACHINE.mem_bytes;
  const bytes = device.handheld ? Math.min(...sizes) : Math.max(...sizes);
  // Turns paced to the display, its numbers the kernel's own (pacing.json).
  const pacing = new FramePacing(
    {
      pace_hz: PACE_HZ,
      margin_share: MARGIN_SHARE,
      batch_frames: BATCH_FRAMES,
      settle_ms: SETTLE_MS,
    },
    (run) => requestAnimationFrame(run),
  );
  const runtime = await boot_runtime(MACHINE, {
    core: source({ bytes }),
    fs: namespace(),
    onabort,
    gpu,
    program,
    policy: pacing,
    submit: submit_grant,
    onterminated,
  });
  return { runtime, pacing };
}
