// @xpute/native

/**
 * xpute's native module, which an Expo app autolinks (ios/, android/): the
 * guest linked into the app, as @xpute/host's native platform opens it; its
 * GPU host, as @xpute/gpu's `NativeGpu` grants it; and the view a guest's
 * surface draws into. The guest is the app's core crate, built as the app is
 * (script/build-core.py) from what its package.json's `xpute.core` names.
 *
 * The guest's memory is one ArrayBuffer over the region it took, with no
 * copy. Each call runs the guest on its own thread while the JavaScript
 * thread waits, and a hostcall it makes is answered on the JavaScript thread
 * meanwhile.
 */

import type { f64, i32, u32 } from "@xpute/kit/abi/word.ts";
import { requireNativeModule, requireNativeView } from "expo";
import type { ComponentType } from "react";
import type { NativeSyntheticEvent, ViewProps } from "react-native";

interface Xpute {
  /** `onWake`: the GPU host asks a turn, for a completion it learned of
   * between turns. */
  addListener(event: "onWake", listener: () => void): { remove(): void };
  mem(): ArrayBuffer;
  /** A turn of `quotaMs`, and when the guest wants the next, in ms; NaN once
   * it has terminated. */
  run(quotaMs: f64): f64;
  /** `answer` answers the guest's hostcalls from here on. */
  hostcallOn(answer: (at: u32) => i32): void;
  /** Whether a device opened; what it is, the guest reads from its host. */
  gpuOpen(): boolean;
}

export const XPUTE: Xpute = requireNativeModule<Xpute>("Xpute");

/** A view the guest's surface `surface` draws into, 0 for none. It says when
 * the surface is bound (`onAttach`), which comes after mount, and when it is
 * let go (`onDetach`). The guest sizes and draws it inside its turns. */
export interface SurfaceViewProps extends ViewProps {
  surface: u32;
  onAttach: (e: NativeSyntheticEvent<{ surface: u32 }>) => void;
  onDetach: () => void;
}

export const SurfaceView: ComponentType<SurfaceViewProps> =
  requireNativeView("Xpute");
