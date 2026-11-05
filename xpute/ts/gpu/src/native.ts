// @xpute/gpu/native.ts

/**
 * A native app's GPU, granted to a guest (@xpute/host `Gpu`). The device and
 * its host run in the app's own process (xpute-gpu host native), reached
 * through xpute's native module (@xpute/native), which runs the guest's
 * stream as the guest publishes it and kicks the guest for a completion;
 * here the device is opened and the guest is told it answered.
 */

import type { Gpu, GpuGrant } from "@xpute/host";
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

  /** Run as the guest publishes it, natively. */
  after_turn(): void {}

  private tell(state: GpuState): void {
    this.state = state;
    this.report(state);
  }
}
