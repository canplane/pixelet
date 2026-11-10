// @xpute/gpu/native.ts

/**
 * This is a native app's GPU, granted to a core (gpu.ts `Gpu`). The
 * device and its runtime run in the app's own process (xpute-gpu runtime
 * native), reached through this package's module. That runtime
 * runs the core's stream as the core publishes it, and kicks the core when a
 * request completes. This class only opens the device and tells the core that
 * it answered.
 */

import type { Gpu, GpuGrant, GpuState } from "./gpu.ts";
import type { u32 } from "@xpute/kit/abi/word.ts";
import { requireNativeModule, requireNativeView } from "expo";
import type { ComponentType } from "react";
import type { NativeSyntheticEvent, ViewProps } from "react-native";

interface XputeGpu {
  /** This says whether a GPU device opened. What the device is, the core reads
   * from its GPU runtime. */
  open(): boolean;
}

const XPUTE_GPU = requireNativeModule<XputeGpu>("XputeGpu");

export class NativeGpu implements Gpu {
  private readonly _reports: ((state: GpuState) => void)[] = [];
  private state: GpuState | null = null;

  watch(report: (state: GpuState) => void): void {
    this._reports.push(report);
    if (this.state !== null) report(this.state);
  }

  grant(to: Pick<GpuGrant, "ready">): void {
    if (!XPUTE_GPU.open()) {
      console.error("[gpu] no device opened; nothing can be drawn");
      this.tell("missing");
      return;
    }
    this.tell("present");
    to.ready();
  }

  private tell(state: GpuState): void {
    this.state = state;
    for (const report of this._reports) report(state);
  }
}

/** The core's surface `surface` draws into this view, and 0 names no surface.
 * The view tells when the surface is bound (`onAttach`), which happens after
 * mount, and when it is let go (`onDetach`). The core sizes the surface and
 * draws it inside its turns. */
export interface SurfaceViewProps extends ViewProps {
  surface: u32;
  onAttach: (e: NativeSyntheticEvent<{ surface: u32 }>) => void;
  onDetach: () => void;
}

export const SurfaceView: ComponentType<SurfaceViewProps> =
  requireNativeView("XputeGpu");
