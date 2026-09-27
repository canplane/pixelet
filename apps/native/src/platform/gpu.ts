// @pxl/native/platform/gpu.ts

/**
 * The device the kernel draws with, on a device: the core's own, opened
 * through wgpu (crates/pxl/web/render/src/gpu/api/device.rs) and announced to
 * the kernel as the page's WebGPU is (platform/web/gpu.ts).
 */

import { Command, encode_gpu_create } from "@/kernel/abi/cmd.spec.ts";
import { CORE_DOORBELL } from "@/kernel/mem.ts";
import { request_frame } from "@/kernel/sched.ts";
import { PLATFORM } from "./module.ts";

export function gpuBoot(): void {
  const info = PLATFORM.gpuOpen();
  if (info === null) {
    // The app installs only where the system draws through Metal or Vulkan,
    // so this is the system failing, not a device the app does not support:
    // there is nothing to tell a reader, and nothing to draw.
    console.error("[gpu] the system opened no GPU; nothing can be drawn");
    return;
  }
  const [align, timestamps] = info;
  CORE_DOORBELL.send(0, Command.GPU_CREATE, encode_gpu_create({ min_uniform_offset_alignment: align, timestamps: timestamps !== 0 }));
  request_frame();
}
