// @xpute/gpu-native-runtime

/**
 * This is a native app's GPU, granted to a guest (gpu.ts `Gpu`). The
 * device and its host run in the app's own process (xpute-gpu-native-host),
 * reached through this package's module. That host runs the guest's stream
 * as each entry returns, and kicks the guest when a request completes. This
 * class opens a device, tells the guest that it answered, and opens one again
 * after a loss or a refusal, as a page's GPU asks again (web.ts `WebGpu`).
 */

import {
  Acquirer,
  type Gpu,
  type GpuGrant,
  type GpuState,
} from "@xpute/gpu-link";
import type { u32 } from "@xpute/kit/abi/word.ts";
import { requireNativeModule, requireNativeView } from "expo";
import type { ComponentType } from "react";
import type { NativeSyntheticEvent, ViewProps } from "react-native";

interface XputeGpu {
  /** This says whether a GPU device is open, newly or already; after a loss
   * it opens a new one. What the device is, the guest reads from its GPU
   * host. */
  open(): boolean;
  /** `onLost` fires once each time the open device is lost. The host has
   * told the guest and let the device go, so the next `open` opens a new one,
   * and the surfaces bound stay bound to it. */
  addListener(event: "onLost", listener: () => void): { remove(): void };
}

const XPUTE_GPU = requireNativeModule<XputeGpu>("XputeGpu");

export class NativeGpu implements Gpu {
  /** These are told of the GPU's state, and `state` is the last one told. */
  private readonly _reports: ((state: GpuState) => void)[] = [];
  private state: GpuState | null = null;

  watch(report: (state: GpuState) => void): void {
    this._reports.push(report);
    if (this.state !== null) report(this.state);
  }

  /** An app in the background draws nothing, so an ask waits until the guest
   * is shown (`GpuGrant.shown`, the link's `shown`). The host says the loss
   * itself (xpute-gpu-native-host lib.rs `GpuHost::new`), so this class only
   * asks for a new device. */
  grant(to: Pick<GpuGrant, "ready" | "shown">): void {
    const acquirer = new Acquirer({
      acquire: async () => XPUTE_GPU.open(),
      tell: (state) => this.tell(state),
      ready: () => to.ready(),
      shown: () => to.shown(),
    });
    XPUTE_GPU.addListener("onLost", () => acquirer.lost());
    acquirer.request();
  }

  private tell(state: GpuState): void {
    this.state = state;
    for (const report of this._reports) report(state);
  }
}

/** The guest's surface `surface` draws into this view, and 0 names no surface.
 * The view tells when the surface is bound (`onAttach`), which happens after
 * mount, and when it is let go (`onDetach`). The guest sizes the surface and
 * draws it inside its entries. */
export interface SurfaceViewProps extends ViewProps {
  surface: u32;
  onAttach: (e: NativeSyntheticEvent<{ surface: u32 }>) => void;
  onDetach: () => void;
}

export const SurfaceView: ComponentType<SurfaceViewProps> =
  requireNativeView("XputeGpu");
