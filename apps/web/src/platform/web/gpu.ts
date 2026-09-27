// @pxl/web/platform/web/gpu.ts

/**
 * The device the kernel draws with, on the web: the page's WebGPU, asked for
 * once the kernel has booted and again after one is lost, and the kernel told
 * of each (webgpu.ts runs the calls it records). A device's core draws
 * through its own and is told nothing from here.
 */

import { Command, encode_gpu_create } from "@/kernel/abi/cmd.spec.ts";
import { CORE_DOORBELL } from "@/kernel/mem.ts";
import { acquireDevice } from "@/platform/web/webgpu.ts";
import { request_frame } from "@/kernel/sched.ts";

/** How long to wait before asking again when no adapter answers while the
 * page is shown. Only a device lost after boot gets here — a browser with
 * none never boots (main.tsx) — and there is no telling when a browser will
 * give one again: Safari, its GPU process lost, gave none for a while after,
 * to this tab and every other. So the page keeps asking. */
const RETRY_MS = 1000;

/** Asks again once the page is shown, or after RETRY_MS if it is: a hidden
 * page draws nothing, so there is nothing a device is wanted for until it is
 * shown again. */
function askAgain(): void {
  if (document.visibilityState === "visible") {
    console.warn(`[gpu] no adapter answered; asking again in ${RETRY_MS} ms`);
    setTimeout(gpuBoot, RETRY_MS);
    return;
  }
  const shown = () => {
    if (document.visibilityState !== "visible") return;
    document.removeEventListener("visibilitychange", shown);
    gpuBoot();
  };
  document.addEventListener("visibilitychange", shown);
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
    if (info === null) return askAgain();
    CORE_DOORBELL.send(0, Command.GPU_CREATE, encode_gpu_create({ min_uniform_offset_alignment: info.align, timestamps: info.timestamps }));
    request_frame();
  }, (err) => console.error("[gpu] the device request failed; nothing can be drawn:", err));
}
