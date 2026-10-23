// @pxl/native/platform/gpu.ts

import { Command } from "@/kernel/abi/cmd.spec.ts";
import { request_frame, send_next } from "@/kernel/sched.ts";
import { Gpu, setGpu } from "@/kernel/gpu.ts";
import { PLATFORM } from "./module.ts";

export function gpuBoot(): void {
  if (!PLATFORM.gpuOpen()) {
    // The app installs only where Metal or Vulkan exists, so this is the system failing.
    console.error("[gpu] the system opened no GPU; nothing can be drawn");
    setGpu(Gpu.Missing);
    return;
  }
  // A completion the device's host learns of between turns asks the turn
  // that reads it, as the page's host does (platform/web/gpu.ts).
  PLATFORM.addListener("onWake", request_frame);
  send_next(Command.GPU_READY);
}
