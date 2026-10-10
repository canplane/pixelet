// @pxl/native/platform/gpu.ts

import { Command, encode_gpu_ready } from "@/kernel/abi/cmd.spec.ts";
import { send_next } from "@/kernel/sched.ts";
import { gpuAbsent } from "@/kernel/gpu.ts";
import { PLATFORM } from "./module.ts";

export function gpuBoot(): void {
  const info = PLATFORM.gpuOpen();
  if (info === null) {
    // The app installs only where Metal or Vulkan exists, so this is the system failing.
    console.error("[gpu] the system opened no GPU; nothing can be drawn");
    gpuAbsent(true);
    return;
  }
  const [align, timestamps] = info;
  send_next(Command.GPU_READY, encode_gpu_ready({ min_uniform_offset_alignment: align, timestamps: timestamps !== 0 }));
}
