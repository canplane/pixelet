// @pxl/web/platform/web/gpu.ts

import { Command, encode_gpu_ready } from "@/kernel/abi/cmd.spec.ts";
import { acquireDevice } from "@/platform/web/webgpu.ts";
import { send_next } from "@/kernel/sched.ts";
import { Gpu, setGpu } from "@/kernel/gpu.ts";

/** Safari, after losing its GPU process, gave no device for a while, so the
 * page keeps asking. The first retry waits too, or an adapter returning an
 * already-lost device loops at once. A fixed interval, not a backoff: the GPU
 * is this device's alone, so there are no other askers to spread out. */
const RETRY_MS = 1000;

/** Asks before the GPU is given up, about ten seconds of them. */
const TRIES = 10;

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
    setGpu(Gpu.Waiting);
    send_next(Command.GPU_LOST);
    askAgain();
  };
  tries += 1;
  void acquireDevice(lost).then((info) => {
    if (info === null) {
      if (tries === 1) console.warn(`[gpu] no adapter answered; asking every ${RETRY_MS} ms while the page is shown`);
      if (tries >= TRIES) {
        console.error(`[gpu] no adapter answered in ${tries} requests; nothing can be drawn`);
        setGpu(Gpu.Missing);
        return;
      }
      setGpu(Gpu.Waiting);
      return askAgain();
    }
    setGpu(Gpu.Present);
    if (tries > 1) console.info(`[gpu] a device answered after ${tries} requests`);
    tries = 0;
    send_next(Command.GPU_READY, encode_gpu_ready({ min_uniform_offset_alignment: info.align, timestamps: info.timestamps }));
  }, (err) => {
    console.error("[gpu] the device request failed; nothing can be drawn:", err);
    setGpu(Gpu.Missing);
  });
}
