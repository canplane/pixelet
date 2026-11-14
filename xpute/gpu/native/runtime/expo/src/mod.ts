// @xpute/gpu-native-runtime

/**
 * This is the GPU's module, which an Expo app autolinks (ios/, android/)
 * beside xpute's (@xpute/native-runtime). The GPU's host runs in the app's
 * own process (xpute-gpu-native-host) and opens its device when the guest
 * asks for one, as a page's GPU host does (web.ts `WebGpu`). The module
 * tells the host of each display frame from the platform's display link
 * itself (Swift, Kotlin), tells of a device lost (`onLost`), and gives the
 * view a guest's surface draws into (`SurfaceView`).
 */

import type { u32 } from "@xpute/kit/abi/word.ts";
import { requireNativeModule, requireNativeView } from "expo";
import type { ComponentType } from "react";
import type { NativeSyntheticEvent, ViewProps } from "react-native";

interface XputeGpu {
  /** `onLost` fires once each time a device is lost and let go. The host has
   * told the guest, which asks for a device again, and the surfaces bound
   * stay bound to the next device. */
  addListener(event: "onLost", listener: () => void): { remove(): void };
}

export const XPUTE_GPU: XputeGpu = requireNativeModule<XputeGpu>("XputeGpu");

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
