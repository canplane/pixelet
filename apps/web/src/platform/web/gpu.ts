// @pxl/web/platform/web/gpu.ts

import { Command, encode_gpu_ready } from "@/kernel/abi/cmd.spec.ts";
import { CORE_DOORBELL } from "@/kernel/mem.ts";
import { acquireDevice } from "@/platform/web/webgpu.ts";
import { request_frame } from "@/kernel/sched.ts";

/** Safari, after losing its GPU process, gave no device for a while, so the
 * page keeps asking. The first retry waits too, or an adapter returning an
 * already-lost device loops at once. */
const RETRY_MS = 1000;

/** So an outage is logged once at start and once at end. */
let tries = 0;

/** A hidden page draws nothing, so it waits to be shown. */
function askAgain(): void {
  if (document.visibilityState === "visible") {
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

export function gpuBoot(): void {
  const lost = () => {
    CORE_DOORBELL.send(0, Command.GPU_LOST);
    askAgain();
    request_frame();
  };
  tries += 1;
  void acquireDevice(lost).then((info) => {
    if (info === null) {
      if (tries === 1) console.warn(`[gpu] no adapter answered; asking every ${RETRY_MS} ms while the page is shown`);
      return askAgain();
    }
    if (tries > 1) console.info(`[gpu] a device answered after ${tries} requests`);
    tries = 0;
    CORE_DOORBELL.send(0, Command.GPU_READY, encode_gpu_ready({ min_uniform_offset_alignment: info.align, timestamps: info.timestamps }));
    request_frame();
  }, (err) => console.error("[gpu] the device request failed; nothing can be drawn:", err));
}
