// @pxl/client/runtime/start.ts

/** This is the core as xpute's runtime boots it where the core's memory is: on
 * a device's runtime thread or in a page's worker. */

import { FramePacing } from "@xpute/runtime";
import { type Config, type FrameGrant } from "@xpute/runtime";
import * as text from "@/abi/text.spec.ts";
import { MANIFEST } from "@/abi/manifest.spec.ts";
import { LIMIT } from "@/abi/perf.spec.ts";
import {
  BATCH_FRAMES,
  MARGIN_SHARE,
  PACE_HZ,
  SETTLE_MS,
} from "@/abi/pacing.spec.ts";
import { binding } from "@/service/sys/fs/binding.ts";
import { text_layout, text_raster } from "@/service/core/text.ts";
import { source } from "@/service/platform/core.ts";
import { after_turn } from "@/service/platform/gpu.ts";

/** The core's text calls, answered from the platform's text (service/core/
 * text.ts); every other of the program's commands is the UI's. */
const program = [
  text.text_imports({
    [text.Command.TEXT_LAYOUT]: text_layout,
    [text.Command.TEXT_RASTER]: text_raster,
  }),
];

export function config(handheld: boolean): Config<FrameGrant> {
  // A handheld device gets the smaller memory, because JavaScriptCore counts
  // the whole reservation as the page's memory.
  const sizes = MANIFEST.mem_bytes;
  const pacing = new FramePacing({
    pace_hz: PACE_HZ,
    margin_share: MARGIN_SHARE,
    batch_frames: BATCH_FRAMES,
    settle_ms: SETTLE_MS,
  });
  return {
    core: source({
      bytes: handheld ? Math.min(...sizes) : Math.max(...sizes),
    }),
    fs: binding(),
    after_turn,
    // A build made for measuring says a turn slower than perf.json's
    // LIMIT.SLOW_MS, as the core has a device's runtime do (pxl-core lib.rs
    // `GPU`) and the worker the page's GPU runtime (runtime/worker.ts).
    slow_ms: __PERF_DRIVE__ ? LIMIT.SLOW_MS : undefined,
    policy: pacing,
    submit: () => {},
    program,
  };
}
