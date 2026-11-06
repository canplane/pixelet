// @pxl/client/shell/cover.ts

import { Message } from "@/abi/cmd.spec.ts";
import {
  BOOT_FAILED,
  BOOT_LOADING,
  BOOT_OPENING,
  BOOT_STARTED,
  BOOT_STARTING,
  BOOT_TERMINATED,
  type Boot,
  GPU_ASKING,
  GPU_INSECURE,
  GPU_LOST,
  GPU_MISSING,
  GPU_UNSUPPORTED,
  GPU_WAITING,
  type Gpu,
  SOURCE_FAILING,
  SOURCE_REFUSED,
  type Source,
} from "@/abi/state.ts";
import { messageKey } from "@/shell/locale/message-key.ts";

/** Where the app stands, as what may stop it says. */
export interface Standing {
  boot: Boot;
  gpu: Gpu;
  source: Source;
  online: boolean;
  /** Why the server does not answer the world's reads now. */
  stall: Message | null;
  stopCode: string | null;
  /** How much of what the renderer makes is made, 0 to 1. */
  made: number;
  /** How much of the view's ground shows, 0 to 1. */
  area: number;
  /** The view has a place: the kernel has heard where, or gave up on the location. */
  placed: boolean;
}

/** What stands over the app, `of` the locale key its words are under: progress,
 * while something it waits on may yet come; failed, where it will not; a panic,
 * with the stop code a screenshot sent back needs. */
export type Cover =
  | { kind: "progress"; of: string; share: number }
  | { kind: "failed"; of: string }
  | { kind: "panic"; of: string; code: string | null };

const progress = (s: Standing, of: string): Cover => ({
  kind: "progress",
  of,
  share: (s.made + s.area) / 2,
});
const failed = (of: string): Cover => ({ kind: "failed", of });
const panic = (of: string, code: string | null): Cover => ({
  kind: "panic",
  of,
  code,
});

/** The shell itself fell over, which no state of the boot says. */
export const SHELL_PANIC = panic("panic.app", "SHELL_ERROR");

/** What breaks the world once it is made or being made. */
function panicOf(s: Standing): Cover | null {
  if (s.gpu === GPU_LOST) return panic("panic.gpu", "GPU_LOST");
  if (s.stall === Message.WorldUnavailable) {
    return panic(messageKey(s.stall), "WORLD_UNAVAILABLE");
  }
  return null;
}

/** What keeps the app from showing, by what to say first: failed, where it will
 * not come; progress, while it may. */
function blockOf(s: Standing): Cover | null {
  switch (s.gpu) {
    case GPU_MISSING:
    case GPU_LOST:
      return failed("failed.gpu");
    case GPU_UNSUPPORTED:
      return failed("failed.webgpu");
    case GPU_INSECURE:
      return failed("failed.insecure");
  }
  if (s.source === SOURCE_REFUSED) return failed("failed.source");
  if (!s.online) return progress(s, "progress.offline");
  if (s.source === SOURCE_FAILING || s.stall === Message.WorldReadFailing) {
    return progress(s, "progress.server");
  }
  if (s.gpu === GPU_ASKING || s.gpu === GPU_WAITING) {
    return progress(s, "progress.gpu");
  }
  return null;
}

/** What the world being made waits on, once nothing blocks it: the pipelines
 * the renderer makes as soon as a GPU answers, the source asked for beside
 * them; then the view's place and its area. */
function waitOf(s: Standing): Cover {
  if (s.made < 1) return progress(s, "progress.shaders");
  return progress(s, s.placed ? "progress.area" : "progress.location");
}

/** The one place that says it, step by step: the core opened, a GPU and the
 * source asked for, the world made. Broken once the kernel ran, a panic.
 * Before the app shows, failed or progress, which holds until the boot shows
 * it (abi/boot.ts), so neither ever comes over the app. None once it shows. */
export function coverOf(s: Standing): Cover | null {
  switch (s.boot) {
    case BOOT_TERMINATED:
      return panic("panic.app", s.stopCode);
    case BOOT_FAILED:
      return failed("failed.boot");
    case BOOT_STARTED:
      return panicOf(s);
    case BOOT_LOADING:
      return panicOf(s) ?? blockOf(s) ?? waitOf(s);
    case BOOT_STARTING:
      return blockOf(s) ?? waitOf(s);
    case BOOT_OPENING:
      return progress(s, "progress.app");
  }
}
