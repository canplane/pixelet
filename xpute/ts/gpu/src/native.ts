// @xpute/gpu/native.ts

/**
 * A native app's GPU, granted to a core (@xpute/runtime `Gpu`). The device and
 * its runtime run in the app's own process (xpute-gpu runtime native), reached
 * through xpute's native module (@xpute/native), which runs the core's
 * stream as the core publishes it and kicks the core for a completion;
 * here the device is opened and the core is told it answered.
 */

import type { Gpu, GpuGrant } from "@xpute/runtime";
import { XPUTE } from "@xpute/native";
import type { GpuState } from "./web.ts";

export class NativeGpu implements Gpu {
  private report: (state: GpuState) => void = () => {};
  private state: GpuState | null = null;

  /** `report` is told whether there is a GPU, the last word at once. */
  watch(report: (state: GpuState) => void): void {
    this.report = report;
    if (this.state !== null) report(this.state);
  }

  grant(to: GpuGrant): void {
    if (!XPUTE.gpuOpen()) {
      console.error("[gpu] no device opened; nothing can be drawn");
      this.tell("missing");
      return;
    }
    this.tell("present");
    to.ready();
  }

  /** Run as the core publishes it, natively. */
  after_turn(): void {}

  private tell(state: GpuState): void {
    this.state = state;
    this.report(state);
  }
}
