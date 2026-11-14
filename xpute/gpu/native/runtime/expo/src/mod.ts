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
}

const XPUTE_GPU: XputeGpu = requireNativeModule<XputeGpu>("XputeGpu");

/** Adds `hear` to what is told what the host has of a GPU, as it changes,
 * over the module's `onState`, which xpute_gpu.h `xpute_gpu_state_on` sends. */
export function state_on(hear: (state: GpuState) => void): { remove(): void } {
  return XPUTE_GPU.addListener("onState", ({ state }) => hear(state));
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
