// @pxl/native/gpu.ts

import type { GpuHeard } from "@pxl/client/bracket.tsx";
import { PLATFORM } from "./module.ts";

export function boot(heard: GpuHeard): void {
  if (!PLATFORM.gpuOpen()) {
    // No device opened as the kernel asks (pxl-native surface.rs `ASK`); what
    // refused it is in the native log (surface.rs `refused`).
    console.error("[gpu] no device opened; nothing can be drawn");
    heard.state("missing");
    return;
  }
  // A completion the device's host learns of between turns asks the turn
  // that reads it.
  PLATFORM.addListener("onWake", () => heard.wake());
  heard.ready();
}
