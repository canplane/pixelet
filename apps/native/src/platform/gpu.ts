// @pxl/native/platform/gpu.ts

/**
 * The device the kernel draws with, on a device: the core's own, opened
 * through wgpu (crates/pxl/web/render/src/gpu/api/device.rs) and announced to
 * the kernel as the page's WebGPU is (platform/web/gpu.ts). A device the
 * system does not give is said once, in front of everything.
 */

import { Command, encode_gpu_create } from "@/kernel/abi/cmd.spec.ts";
import { CORE_BUILD } from "@/kernel/ipc.ts";
import { request_frame } from "@/kernel/sched.ts";
import { declareUnsupported } from "@/app/store/unsupported-store.ts";
import { PXL_KERNEL } from "./core.ts";

export function gpu_boot(): void {
  const info = PXL_KERNEL.gpuOpen();
  if (info === null) {
    console.error("[gpu] the system has no GPU; nothing can be drawn");
    declareUnsupported("gpu");
    return;
  }
  const [align, timestamps] = info;
  CORE_BUILD.send(0, Command.GPU_CREATE, encode_gpu_create({ min_uniform_offset_alignment: align, timestamps: timestamps !== 0 }));
  request_frame();
}
