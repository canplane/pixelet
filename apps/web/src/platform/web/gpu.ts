// @pxl/web/platform/web/gpu.ts

/**
 * The device the kernel draws with, on the web: the page's WebGPU, asked for
 * once the kernel has booted and again after one is lost, and the kernel told
 * of each (render/gpu.ts runs the calls it records). A device's core draws
 * through its own and is told nothing from here.
 */

import { Command, encode_gpu_create } from "@/kernel/abi/cmd.spec.ts";
import { CORE_BUILD } from "@/kernel/ipc.ts";
import { acquire_device } from "@/kernel/render/gpu.ts";
import { request_frame } from "@/kernel/sched.ts";
import { declareUnsupported } from "@/app/store/unsupported-store.ts";

/** Asks for a device and tells the kernel when one answers, and again after
 * one is lost. */
export function gpu_boot(): void {
  const lost = () => {
    CORE_BUILD.send(0, Command.GPU_DELETE);
    gpu_boot();
    request_frame();
  };
  void acquire_device(lost).then((info) => {
    if ("reason" in info) {
      // The same end as a browser that cannot read the wire: nothing here is a
      // moment to recover from, so it is said once, in front of everything,
      // rather than left to a console nobody on that browser will open
      // (app/unsupported.tsx).
      console.error(`[gpu] ${info.detail}; nothing can be drawn`);
      declareUnsupported(info.reason);
      return;
    }
    CORE_BUILD.send(0, Command.GPU_CREATE, encode_gpu_create({ min_uniform_offset_alignment: info.align, timestamps: info.timestamps }));
    request_frame();
  }, (err) => console.error("[gpu] the device request failed; nothing can be drawn:", err));
}
