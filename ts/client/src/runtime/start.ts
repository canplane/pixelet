// @pxl/client/runtime/start.ts

/**
 * The kernel as xpute's runtime boots it, where the kernel's memory is (a
 * device's shell thread, a page's worker): its module, its files, the GPU, the
 * frame pacing and each turn's grant. The kernel's calls it answers there, by
 * a table: the text, and the snapshot, whose refused files are followed where
 * the files are; and the profiler's reads, made here.
 */

import type { BranchCursor } from "@xpute/kit/wire/xtp/mod.ts";
import type { i32 } from "@xpute/kit/abi/word.ts";
import type { U8Array } from "@xpute/kit/abi/array.ts";
import { FramePacing } from "@xpute/runtime";
import {
  mem,
  type Config,
  type FrameGrant,
  type Manifest,
} from "@xpute/runtime";
import {
  Command,
  read_shell_snapshot,
  read_text_layout,
  read_text_raster,
  shell_mem,
} from "@/abi/cmd.spec.ts";
import { MACHINE } from "@/abi/machine.spec.ts";
import {
  BATCH_FRAMES,
  MARGIN_SHARE,
  PACE_HZ,
  SETTLE_MS,
} from "@/abi/pacing.spec.ts";
import { namespace } from "@/sys/fs/namespace.ts";
import { follow_refused_files } from "@/sys/fs/refusal.ts";
import { submit_grant } from "@/port/surface.ts";
import { text_layout, text_raster } from "@/port/text.ts";
import * as snapshot from "@/port/snapshot.ts";
import { answer, type PerfRead } from "@/port/perf.ts";
import type { Perf } from "@/runtime/kernel.ts";
import * as device from "@/service/device.ts";
import { source } from "@/service/kernel.ts";
import { gpu } from "@/service/gpu.ts";

/** The snapshot's bytes, where its frame says they lie. */
function bytes(f: BranchCursor): U8Array {
  const { at, len } = read_shell_snapshot(f);
  return new Uint8Array(mem, at, len);
}

/** What the kernel calls on this side. */
const calls = new Map<number, (f: BranchCursor) => i32 | void>([
  [Command.TEXT_LAYOUT, (f) => text_layout(read_text_layout(f))],
  [Command.TEXT_RASTER, (f) => text_raster(read_text_raster(f))],
  [
    Command.SHELL_SNAPSHOT,
    (f) => {
      snapshot.set_snapshot(bytes(f));
      follow_refused_files(snapshot.current().refused_files);
    },
  ],
]);

export function config(): Config<FrameGrant> {
  // A phone gets less: JavaScriptCore counts the whole reservation as page memory.
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
  return {
    core: source({
      bytes: device.handheld ? Math.min(...sizes) : Math.max(...sizes),
    }),
    fs: namespace(),
    gpu,
    policy: pacing,
    submit: submit_grant,
    program: (_tag, cmd, f) => calls.get(cmd)?.(f!),
    mem_of: shell_mem,
  };
}

export const manifest: Manifest<FrameGrant> = {
  machine: MACHINE,
  core: config,
};

/** The profiler read beside the kernel's memory. */
export const perf_here: Perf = (what, args) =>
  Promise.resolve(answer(what as PerfRead, args));
