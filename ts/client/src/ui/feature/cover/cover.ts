// @pxl/client/ui/feature/cover/cover.ts

import {
  type Message,
  MESSAGE_WORLD_READ_FAILING,
  MESSAGE_WORLD_UNAVAILABLE,
} from "@/abi/overlay.enum.ts";
import {
  BOOT_LOADING,
  BOOT_OPENING,
  BOOT_STARTED,
  BOOT_STARTING,
  BOOT_TERMINATED,
  type Boot,
} from "@/service/sys/boot.ts";
import {
  GPU_ASKING,
  GPU_INSECURE,
  GPU_MISSING,
  GPU_UNSUPPORTED,
  GPU_WAITING,
  type Gpu,
} from "@/service/sys/gpu.ts";
import {
  SOURCE_FAILING,
  SOURCE_REFUSED,
  type Source,
} from "@/service/sys/fs/source.ts";
import { messageKey } from "@/ui/shared/locale/message-key.ts";

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
  progress: number;
  /** The share of the view's ground that shows, from 0 to 1. */
  area: number;
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
  share: (s.progress + s.area) / 2,
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
  if (s.stall === MESSAGE_WORLD_UNAVAILABLE) {
    return panic(messageKey(s.stall), "WORLD_UNAVAILABLE");
  }
  return null;
}

/** What keeps the app from showing, in the order it is to be told: a failure,
 * where the app will not come, before progress, while it may. */
function blockOf(s: Standing): Cover | null {
  switch (s.gpu) {
    case GPU_MISSING:
      return failed("failed.gpu");
    case GPU_UNSUPPORTED:
      return failed("failed.webgpu");
    case GPU_INSECURE:
      return failed("failed.insecure");
  }
  if (s.source === SOURCE_REFUSED) return failed("failed.source");
  if (!s.online) return progress(s, "progress.offline");
  if (s.source === SOURCE_FAILING || s.stall === MESSAGE_WORLD_READ_FAILING) {
    return progress(s, "progress.server");
  }
  if (s.gpu === GPU_ASKING || s.gpu === GPU_WAITING) {
    return progress(s, "progress.gpu");
  }
  return null;
}

/** What the world waits on while it is made, once nothing blocks it. First
 * come the pipelines, which the renderer makes as soon as a GPU answers, with
 * the source asked for beside them. Then comes the view's area. */
function waitOf(s: Standing): Cover {
  if (s.progress < 1) return progress(s, "progress.shaders");
  return progress(s, "progress.area");
}

/** The one place that decides what covers the app, through the boot's steps:
 * the core opened, a GPU and the source asked for, the world made. Once the
 * core has run, what breaks it is a panic. Before the app shows, the cover is
 * a failure or progress, and it holds until the boot shows the app
 * (runtime/ui.ts), so neither ever comes up over the app. Once the app
 * shows, nothing but a panic covers it. */
export function coverOf(s: Standing): Cover | null {
  switch (s.boot) {
    case BOOT_TERMINATED:
      return panic("panic.app", s.stopCode);
    case BOOT_STARTED:
      return panicOf(s);
    case BOOT_LOADING:
    case BOOT_STARTING:
      return panicOf(s) ?? blockOf(s) ?? waitOf(s);
    case BOOT_OPENING:
      return progress(s, "progress.app");
  }
}
