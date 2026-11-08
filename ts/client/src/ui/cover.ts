// @pxl/client/ui/cover.ts

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
import { messageKey } from "@/ui/locale/message-key.ts";

/** Where the app stands, as told by each part that can hold it back or stop
 * it. */
export interface Standing {
  boot: Boot;
  gpu: Gpu;
  source: Source;
  online: boolean;
  /** Why the server is not answering the world's reads, or null while it
   * answers. */
  stall: Message | null;
  stopCode: string | null;
  /** The share of the renderer's pipelines made so far, from 0 to 1. */
  made: number;
  /** The share of the view's ground that shows, from 0 to 1. */
  area: number;
  /** Whether the view has a place: the core has heard the location, or has
   * given up on it. */
  placed: boolean;
}

/** What covers the app. `of` is the locale key of its words. `progress` shows
 * while what the app waits on may still come, and `failed` where it will not.
 * `panic` carries the stop code, which a screenshot sent back needs. */
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

/** The cover for a crash of the UI itself, which no state of the boot
 * reports. */
export const UI_PANIC = panic("panic.app", "UI_ERROR");

/** The panic for what breaks the world once it is made, or while it is being
 * made. */
function panicOf(s: Standing): Cover | null {
  if (s.gpu === GPU_LOST) return panic("panic.gpu", "GPU_LOST");
  if (s.stall === Message.WorldUnavailable) {
    return panic(messageKey(s.stall), "WORLD_UNAVAILABLE");
  }
  return null;
}

/** What keeps the app from showing, in the order it is to be told: a failure,
 * where the app will not come, before progress, while it may. */
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

/** What the world waits on while it is made, once nothing blocks it. First
 * come the pipelines, which the renderer makes as soon as a GPU answers, with
 * the source asked for beside them. Then come the view's place and its area. */
function waitOf(s: Standing): Cover {
  if (s.made < 1) return progress(s, "progress.shaders");
  return progress(s, s.placed ? "progress.area" : "progress.location");
}

/** The one place that decides what covers the app, through the boot's steps:
 * the core opened, a GPU and the source asked for, the world made. Once the
 * core has run, what breaks it is a panic. Before the app shows, the cover is
 * a failure or progress, and it holds until the boot shows the app
 * (runtime/boot.ts), so neither ever comes up over the app. Once the app
 * shows, nothing but a panic covers it. */
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
