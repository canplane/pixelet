// @pxl/native/gpu.ts

import type { GpuHeard } from "@pxl/client/bracket.tsx";
import { PLATFORM } from "./module.ts";

export function gpuBoot(heard: GpuHeard): void {
  if (!PLATFORM.gpuOpen()) {
    // The app installs only where Metal or Vulkan exists, so this is the system failing.
    console.error("[gpu] the system opened no GPU; nothing can be drawn");
    heard.state("missing");
    return;
  }
  // A completion the device's host learns of between turns asks the turn
  // that reads it, as the page's host does (apps/web/src/gpu.ts).
  PLATFORM.addListener("onWake", () => heard.wake());
  heard.ready();
}
