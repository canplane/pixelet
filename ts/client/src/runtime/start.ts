// @pxl/client/runtime/start.ts

/**
 * The core as xpute's runtime boots it where the core's memory is, on a
 * device's UI thread or in a page's worker: its module, its files, the GPU,
 * the frame pacing and each turn's grant. What the core asks on that side is
 * answered through a table: the text, and the rate, which the pacing keeps.
 * The profiler's reads are made here too.
 */

import type { BranchCursor } from "@xpute/kit/wire/xtp/mod.ts";
import type { i32 } from "@xpute/kit/abi/word.ts";
import { FramePacing } from "@xpute/runtime";
import { type Config, type FrameGrant, type Manifest } from "@xpute/runtime";
import { Command, read_turn_pace, ui_mem } from "@/abi/cmd.spec.ts";
import * as text from "@/abi/text.spec.ts";
import { MACHINE } from "@/abi/machine.spec.ts";
import {
  BATCH_FRAMES,
  MARGIN_SHARE,
  PACE_HZ,
  SETTLE_MS,
} from "@/abi/pacing.spec.ts";
import { namespace } from "@/sys/fs/namespace.ts";
import { submit_grant } from "@/port/surface.ts";
import { text_layout, text_raster } from "@/port/text.ts";
import { answer, type PerfRead } from "@/port/perf.ts";
import type { Perf } from "@/port/core.ts";
import { source } from "@/service/core.ts";
import { gpu } from "@/service/gpu.ts";

/** What the core asks on this side. */
function calls(pacing: FramePacing) {
  return new Map<number, (f: BranchCursor) => i32 | void>([
    [text.Command.TEXT_LAYOUT, (f) => text_layout(text.read_text_layout(f))],
    [text.Command.TEXT_RASTER, (f) => text_raster(text.read_text_raster(f))],
    [Command.TURN_PACE, (f) => pacing.pace(read_turn_pace(f).interval_ms)],
  ]);
}

export function config(handheld: boolean): Config<FrameGrant> {
  // A handheld device gets the smaller memory, because JavaScriptCore counts
  // the whole reservation as the page's memory.
  const sizes = MACHINE.mem_bytes;
  const pacing = new FramePacing(
    {
      pace_hz: PACE_HZ,
      margin_share: MARGIN_SHARE,
      batch_frames: BATCH_FRAMES,
      settle_ms: SETTLE_MS,
    },
    (run) => requestAnimationFrame(run),
  );
  const asked = calls(pacing);
  return {
    core: source({
      bytes: handheld ? Math.min(...sizes) : Math.max(...sizes),
    }),
    fs: namespace(),
    gpu,
    policy: pacing,
    submit: submit_grant,
    program: (_tag, cmd, f) => asked.get(cmd)?.(f!),
    mem_of: ui_mem,
  };
}

export function manifest(handheld: boolean): Manifest<FrameGrant> {
  return { machine: MACHINE, core: () => config(handheld) };
}

/** Makes the profiler's reads here, beside the core's memory. */
export const perf_here: Perf = (what, args) =>
  Promise.resolve(answer(what as PerfRead, args));
