// @xpute/gpu/native.ts

/**
 * A native app's GPU, granted to a core (@xpute/runtime `Gpu`). The device and
 * its runtime run in the app's own process (xpute-gpu runtime native), reached
 * through xpute's native module (@xpute/native). That runtime runs the core's
 * stream as the core publishes it, and kicks the core when a request
 * completes. This class only opens the device and tells the core that it
 * answered.
 */

import type { Gpu, GpuGrant, GpuState } from "@xpute/runtime";
import { XPUTE } from "@xpute/native";

export class NativeGpu implements Gpu {
  private readonly _reports: ((state: GpuState) => void)[] = [];
  private state: GpuState | null = null;

  watch(report: (state: GpuState) => void): void {
    this._reports.push(report);
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

  /** Nothing to do: the native runtime runs the stream as the core publishes
   * it. */
  after_turn(): void {}

  private tell(state: GpuState): void {
    this.state = state;
    for (const report of this._reports) report(state);
  }
}
