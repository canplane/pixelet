// @pxl/client/shell/cover.ts

import { Message } from "@/abi/cmd.spec.ts";
import { Boot, Gpu, Source } from "@/abi/state.ts";
import { messageKey } from "@/shell/locale/message-key.ts";

/** Where the app stands, as what may stop it says. */
export interface Standing {
  boot: Boot;
  gpu: Gpu;
  source: Source;
  online: boolean;
  /** The world's reads failed past every retry (`Message.WorldUnavailable`). */
  givenUp: boolean;
  /** The world's reads are failing and retried (`Message.WorldReadFailing`). */
  readFailing: boolean;
  stopCode: string | null;
  /** How much of what the renderer makes is made, 0 to 1. */
  made: number;
}

/** What stands over the app, `of` the locale key its words are under: progress,
 * while something it waits on may yet come; failed, where it will not; a panic,
 * with the stop code a screenshot sent back needs. */
export type Cover =
  | { kind: "progress" | "failed"; of: string }
  | { kind: "panic"; of: string; code: string | null };

const progress = (of: string): Cover => ({ kind: "progress", of });
const failed = (of: string): Cover => ({ kind: "failed", of });
const panic = (of: string, code: string | null): Cover => ({
  kind: "panic",
  of,
  code,
});

/** The one place that says it. Broken once the kernel ran, a panic. Before the
 * app shows, failed or progress, which holds until the boot shows it
 * (abi/boot.ts), so neither ever comes over the app. None once it shows. */
export function coverOf(s: Standing): Cover | null {
  if (s.boot === Boot.Stopped) return panic("panic.kernel", s.stopCode);
  if (s.boot === Boot.Started || s.boot === Boot.Loading) {
    if (s.gpu === Gpu.Lost) return panic("panic.gpu", "GPU_LOST");
    if (s.givenUp) {
      return panic(messageKey(Message.WorldUnavailable), "WORLD_UNAVAILABLE");
    }
  }
  if (s.boot === Boot.Started) return null;
  if (s.boot === Boot.Failed) return failed("failed.boot");
  if (s.gpu === Gpu.Missing || s.gpu === Gpu.Lost) return failed("failed.gpu");
  if (s.source === Source.Refused) return failed("failed.source");
  if (!s.online) return progress("progress.offline");
  if (s.source === Source.Failing || s.readFailing) {
    return progress("progress.server");
  }
  if (s.gpu === Gpu.Asking || s.gpu === Gpu.Waiting) {
    return progress("progress.gpu");
  }
  if (s.boot === Boot.Starting) return progress("progress.source");
  return progress(s.made < 1 ? "progress.shaders" : "toast.world_loading");
}
