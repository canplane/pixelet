// @pxl/client/runtime/start.ts

/** This is the core as xpute's runtime boots it where the core's memory is: on
 * a device's runtime thread or in a page's worker. */

import type { BranchCursor } from "@xpute/kit/wire/xtp/mod.ts";
import type { i32 } from "@xpute/kit/abi/word.ts";
import { FramePacing } from "@xpute/runtime";
import { type Config, type FrameGrant, type Manifest } from "@xpute/runtime";
import { ui_mem } from "@/abi/cmd.spec.ts";
import * as text from "@/abi/text.spec.ts";
import { MACHINE } from "@/abi/machine.spec.ts";
import {
  BATCH_FRAMES,
  MARGIN_SHARE,
  PACE_HZ,
  SETTLE_MS,
} from "@/abi/pacing.spec.ts";
import { namespace } from "@/service/sys/fs/namespace.ts";
import { text_layout, text_raster } from "@/service/core/text.ts";
import { answer, type PerfRead } from "@/service/core/perf.ts";
import type { Perf } from "@/service/core/core.ts";
import { source } from "@/service/platform/core.ts";
import { gpu } from "@/service/platform/gpu.ts";

function calls() {
  return new Map<number, (f: BranchCursor) => i32 | void>([
    [
      text.Command.TEXT_LAYOUT,
      (f) => text_layout(text.read[text.Command.TEXT_LAYOUT](f)),
    ],
    [
      text.Command.TEXT_RASTER,
      (f) => text_raster(text.read[text.Command.TEXT_RASTER](f)),
    ],
  ]);
}

export function config(handheld: boolean): Config<FrameGrant> {
  // A handheld device gets the smaller memory, because JavaScriptCore counts
  // the whole reservation as the page's memory.
  const sizes = MACHINE.mem_bytes;
  const pacing = new FramePacing({
    pace_hz: PACE_HZ,
    margin_share: MARGIN_SHARE,
    batch_frames: BATCH_FRAMES,
    settle_ms: SETTLE_MS,
  });
  const asked = calls();
  return {
    core: source({
      bytes: handheld ? Math.min(...sizes) : Math.max(...sizes),
    }),
    fs: namespace(),
    gpu,
    policy: pacing,
    submit: () => {},
    program: (_tag, cmd, f) => asked.get(cmd)?.(f!),
    mem_of: ui_mem,
  };
}

export function manifest(handheld: boolean): Manifest<FrameGrant> {
  return { machine: MACHINE, core: () => config(handheld) };
}

export const perf_here: Perf = (what, args) =>
  Promise.resolve(answer(what as PerfRead, args));
