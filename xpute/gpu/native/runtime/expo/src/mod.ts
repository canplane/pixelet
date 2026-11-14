// @xpute/gpu-native-runtime

/**
 * This is the GPU's module, which an Expo app autolinks (ios/, android/)
 * beside xpute's (@xpute/native-runtime). The GPU's host runs in the app's
 * own process (xpute-gpu-native-host) and opens its device when the guest
 * asks for one, as a page's GPU host does (web.ts `WebGpu`). The module
 * tells the host of each display frame from the platform's display link
 * itself (Swift, Kotlin), tells what the host has of a GPU as it changes
 * (`state_on`), as a page's host reports it, and gives the view a guest's
 * surface draws into (`SurfaceView`).
 */

import type { u32 } from "@xpute/kit/abi/word.ts";
import type { GpuState } from "@xpute/gpu-link";
import { requireNativeModule, requireNativeView } from "expo";
import type { ComponentType } from "react";
import type { NativeSyntheticEvent, ViewProps } from "react-native";

interface XputeGpu {
  /** `onState` fires with what the host has of a GPU, as it changes:
   * `present` once the guest's ask finds a device open, `missing` once it
   * finds none opens, and `waiting` once a device is lost, as the guest asks
   * again; the surfaces bound stay bound to the next device. */
  addListener(event: "onState", listener: (e: { state: GpuState }) => void): { remove(): void };
  /** What the host has of a GPU now, or null before the guest's first ask is
   * answered (xpute_gpu.h `xpute_gpu_state`). */
  state(): GpuState | null;
}

const XPUTE_GPU: XputeGpu = requireNativeModule<XputeGpu>("XputeGpu");

/** Adds `hear` to what is told what the host has of a GPU, as it changes,
 * over the module's `onState`, which xpute_gpu.h `xpute_gpu_state_on` sends,
 * and tells it the state now, which a UI started after the host told one has
 * not heard, as a reloaded page hears its new worker's host. A change between
 * the two is heard after the state now. */
export function state_on(hear: (state: GpuState) => void): { remove(): void } {
  const sub = XPUTE_GPU.addListener("onState", ({ state }) => hear(state));
  const cur = XPUTE_GPU.state();
  if (cur !== null) hear(cur);
  return sub;
}

/** The guest's surface `surface` draws into this view, and 0 names no surface.
 * The view tells when the surface is bound (`onAttach`), which happens after
 * mount, and when it is let go (`onDetach`). The guest sizes the surface and
 * draws into it. */
export interface SurfaceViewProps extends ViewProps {
  surface: u32;
  onAttach: (e: NativeSyntheticEvent<{ surface: u32 }>) => void;
  onDetach: () => void;
}

export const SurfaceView: ComponentType<SurfaceViewProps> =
  requireNativeView("XputeGpu");
