// @pxl/web/gpu.ts

import type { GpuHeard } from "@pxl/client/bracket.tsx";
import * as webgpu from "./webgpu.ts";

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

/** Told what each ask hears; kept for the asks after a loss. */
let heard: GpuHeard;

/** A hidden page draws nothing, so it waits to be shown. */
function retry(): void {
  if (document.visibilityState === "visible") {
    setTimeout(ask, RETRY_MS);
    return;
  }
  const shown = () => {
    if (document.visibilityState !== "visible") return;
    document.removeEventListener("visibilitychange", shown);
    ask();
  };
  document.addEventListener("visibilitychange", shown);
}

export function boot(told: GpuHeard): void {
  heard = told;
  ask();
}

function ask(): void {
  // The kernel hears of the loss from the device's host (xpute GpuHost);
  // asking again is the page's.
  const lost = () => {
    heard.state("waiting");
    retry();
  };
  tries += 1;
  // A completion the device writes asks for the turn that reads it (`heard.wake`).
  void webgpu.acquire(lost, () => heard.wake()).then((answered) => {
    if (!answered) {
      if (tries === 1) console.warn(`[gpu] no adapter answered; asking every ${RETRY_MS} ms while the page is shown`);
      if (tries >= TRIES) {
        console.error(`[gpu] no adapter answered in ${tries} requests; nothing can be drawn`);
        heard.state(had ? "lost" : "missing");
        return;
      }
      heard.state("waiting");
      return retry();
    }
    heard.state("present");
    had = true;
    if (tries > 1) console.info(`[gpu] a device answered after ${tries} requests`);
    tries = 0;
    heard.ready();
  }, (err) => {
    console.error("[gpu] the device request failed; nothing can be drawn:", err);
    heard.state(had ? "lost" : "missing");
  });
}
