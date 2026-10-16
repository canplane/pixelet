// @pxl/native/platform/gpu.ts

import { Command } from "@/kernel/abi/cmd.spec.ts";
import { send_next } from "@/kernel/sched.ts";
import { Gpu, setGpu } from "@/kernel/gpu.ts";
import { PLATFORM } from "./module.ts";

export function gpuBoot(): void {
  if (!PLATFORM.gpuOpen()) {
    // The app installs only where Metal or Vulkan exists, so this is the system failing.
    console.error("[gpu] the system opened no GPU; nothing can be drawn");
    setGpu(Gpu.Missing);
    return;
  }
  send_next(Command.GPU_READY);
}
