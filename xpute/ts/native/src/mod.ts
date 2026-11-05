// @xpute/native

/**
 * xpute's native module, which an Expo app autolinks (ios/, android/): the
 * guest linked into the app, as @xpute/host's native platform opens it; its
 * GPU host, as @xpute/gpu's `NativeGpu` grants it; and the view a guest's
 * surface draws into. The guest is the app's core crate, built as the app is
 * (script/build-core.py) from what its package.json's `xpute.core` names.
 *
 * The guest's memory is one ArrayBuffer over the region it took, with no
 * copy. The guest runs on its own thread while it has work (xpute-host
 * native/thread.rs): the JavaScript thread kicks it, waits on it only for a
 * turn it rings (`run`), and answers its hostcalls, there or as it is told
 * one waits (`onNotify`, `answer`).
 */

import type { f64, i32, u32 } from "@xpute/kit/abi/word.ts";
import { requireNativeModule, requireNativeView } from "expo";
import type { ComponentType } from "react";
import type { NativeSyntheticEvent, ViewProps } from "react-native";

interface Xpute {
  /** `onNotify`: the guest has posted or asked since `ack`, a hostcall
   * waits for `answer`, or the guest has stopped. */
  addListener(event: "onNotify", listener: () => void): { remove(): void };
  mem(): ArrayBuffer;
  /** A turn given at least `quotaMs`, waited for, its hostcalls answered
   * here meanwhile: when the guest wants the next, in ms; NaN once it has
   * terminated. Back early where no turn may begin until the completion ring
   * is read. */
  run(quotaMs: f64): f64;
  /** A turn wanted at once, the next given at least `quotaMs`, none of its
   * own where negative; nothing waited for. */
  kick(quotaMs: f64): void;
  /** No turn begins but one `run` rings; a kick and the guest's wake time
   * wait for `resume`. */
  pause(): void;
  /** Turns begin again as kicks and the guest's wake time ask. */
  resume(): void;
  /** The hostcall waiting, if one does, answered here. */
  answer(): void;
  /** What the turns since asked: when the guest wants the next, in ms (0 at
   * once, NaN once it has stopped), then the longest of
   * them, in ms. */
  ack(): [f64, f64];
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
