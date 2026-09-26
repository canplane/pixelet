// @pxl/web/app/store/unsupported-store.ts

import { create } from "zustand";

/**
 * Why this browser cannot run the app, once something finds out.
 *
 * What lands here is a property of the browser rather than of a moment, so
 * there is nothing to retry and nothing to recover to: it is said once and the
 * app stops. That is what tells it from a toast, which is for the things that
 * pass.
 *
 * Only the first reason is kept. A browser found missing one of these is about
 * to be found missing the others, and the second sentence would only replace
 * the one the reader is already reading.
 *
 * Each is named as its sentence is in the locale catalog
 * (`unsupported.<reason>`), which is why its value is a string.
 *
 * They are four reasons and not one sentence with four causes: what is missing
 * is put right in a different place each time — a newer browser, the machine's
 * own graphics, the address the page was opened at — and a reader sent to the
 * wrong one is worse off than a reader told nothing.
 */
export const enum UnsupportedReason {
  /** The map data's compression cannot be read. */
  Encoding = "encoding",
  /** There is no WebGPU. */
  WebGpu = "webgpu",
  /** WebGPU is there but no adapter answered. */
  Gpu = "gpu",
  /** WebGPU is withheld from a page not served securely. */
  Insecure = "insecure",
}

interface UnsupportedState {
  reason: UnsupportedReason | null;
  declare: (reason: UnsupportedReason) => void;
}

export const useUnsupportedStore = create<UnsupportedState>((set) => ({
  reason: null,
  declare: (reason) => set((state) => (state.reason === null ? { reason } : state)),
}));

/** For a caller outside React, which is where these are found out. */
export function declareUnsupported(reason: UnsupportedReason): void {
  useUnsupportedStore.getState().declare(reason);
}

export function isUnsupported(): boolean {
  return useUnsupportedStore.getState().reason !== null;
}
