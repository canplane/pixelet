// @pxl/web/platform/web/gpu.ts

import { Command } from "@/kernel/abi/cmd.spec.ts";
import { acquireDevice } from "./webgpu.ts";
import { request_frame, send_next } from "@/kernel/sched.ts";
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

/** A device answered once: giving up after it is losing the GPU, not lacking one. */
let had = false;

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
  // The kernel hears of the loss from the device's host (xpute GpuHost);
  // asking again is the page's.
  const lost = () => {
    setGpu(Gpu.Waiting);
    askAgain();
  };
  tries += 1;
  // A completion the device writes asks the strobe for the turn that reads it.
  void acquireDevice(lost, request_frame).then((answered) => {
    if (!answered) {
      if (tries === 1) console.warn(`[gpu] no adapter answered; asking every ${RETRY_MS} ms while the page is shown`);
      if (tries >= TRIES) {
        console.error(`[gpu] no adapter answered in ${tries} requests; nothing can be drawn`);
        setGpu(had ? Gpu.Lost : Gpu.Missing);
        return;
      }
      setGpu(Gpu.Waiting);
      return askAgain();
    }
    setGpu(Gpu.Present);
    had = true;
    if (tries > 1) console.info(`[gpu] a device answered after ${tries} requests`);
    tries = 0;
    send_next(Command.GPU_READY);
  }, (err) => {
    console.error("[gpu] the device request failed; nothing can be drawn:", err);
    setGpu(had ? Gpu.Lost : Gpu.Missing);
  });
}
