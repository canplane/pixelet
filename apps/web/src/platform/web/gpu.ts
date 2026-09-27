// @pxl/web/platform/web/gpu.ts

/**
 * The device the kernel draws with, on the web: the page's WebGPU, asked for
 * once the kernel has booted and again after one is lost, and the kernel told
 * of each (webgpu.ts runs the calls it records). A device's core draws
 * through its own and is told nothing from here.
 */

import { Command, encode_gpu_create } from "@/kernel/abi/cmd.spec.ts";
import { CORE_DOORBELL } from "@/kernel/mem.ts";
import { acquireDevice, missingBeforeBoot } from "@/platform/web/webgpu.ts";
import { request_frame } from "@/kernel/sched.ts";
import { declareUnsupported, type UnsupportedReason } from "@/app/store/unsupported-store.ts";

/** Why this browser cannot draw at all, before anything is started: what it
 * lacks outright, where the device's answer need not be waited for. */
export function gpuMissing(): UnsupportedReason | null {
  return missingBeforeBoot();
}

/** Asks for a device and tells the kernel when one answers, and again after
 * one is lost. */
export function gpuBoot(): void {
  const lost = () => {
    CORE_DOORBELL.send(0, Command.GPU_DELETE);
    gpuBoot();
    request_frame();
  };
  void acquireDevice(lost).then((info) => {
    if ("reason" in info) {
      // The same end as a browser that cannot read the wire: nothing here is a
      // moment to recover from, so it is said once, in front of everything,
      // rather than left to a console nobody on that browser will open
      // (app/unsupported.tsx).
      console.error(`[gpu] ${info.detail}; nothing can be drawn`);
      declareUnsupported(info.reason);
      return;
    }
    CORE_DOORBELL.send(0, Command.GPU_CREATE, encode_gpu_create({ min_uniform_offset_alignment: info.align, timestamps: info.timestamps }));
    request_frame();
  }, (err) => console.error("[gpu] the device request failed; nothing can be drawn:", err));
}
