// @pxl/native/platform/gpu.ts

import { Command, encode_gpu_ready } from "@/kernel/abi/cmd.spec.ts";
import { CORE_DOORBELL } from "@/kernel/mem.ts";
import { request_frame } from "@/kernel/sched.ts";
import { PLATFORM } from "./module.ts";

export function gpuBoot(): void {
  const info = PLATFORM.gpuOpen();
  if (info === null) {
    // The app installs only where Metal or Vulkan exists, so this is the system failing.
    console.error("[gpu] the system opened no GPU; nothing can be drawn");
    return;
  }
  const [align, timestamps] = info;
  CORE_DOORBELL.send(0, Command.GPU_READY, encode_gpu_ready({ min_uniform_offset_alignment: align, timestamps: timestamps !== 0 }));
  request_frame();
}
