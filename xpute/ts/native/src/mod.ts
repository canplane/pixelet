// @xpute/native

/**
 * xpute's native module, which an Expo app autolinks (ios/, android/): the
 * core linked into the app, as @xpute/runtime's native platform opens it; its
 * GPU runtime, as @xpute/gpu's `NativeGpu` grants it; and the view a core's
 * surface draws into. The core is the app's core crate, built as the app is
 * (script/build-core.py) from what its package.json's `xpute.core` names.
 *
 * The core's memory is one ArrayBuffer over the region it took, with no
 * copy. The core runs on its own thread while it has work (xpute-runtime
 * native/thread.rs): the JavaScript thread kicks it, waits on it only for a
 * turn it rings (`run`), and answers its calls, there or as it is told
 * one waits (`onNotify`, `answer`).
 */

import type { f64, i32, u32 } from "@xpute/kit/abi/word.ts";
import { requireNativeModule, requireNativeView } from "expo";
import type { ComponentType } from "react";
import type { NativeSyntheticEvent, ViewProps } from "react-native";

interface Xpute {
  /** `onNotify`: the core has posted or asked since `turns`, a call
   * waits for `answer`, or the core has stopped. */
  addListener(event: "onNotify", listener: () => void): { remove(): void };
  mem(): ArrayBuffer;
  /** The word at byte `at` of the memory, loaded with acquire. */
  load(at: u32): u32;
  /** `word` stored at byte `at` of the memory with release. */
  store(at: u32, word: u32): void;
  /** A turn ending no sooner than `deadlineMs` from its rise, waited for, its calls answered
   * here meanwhile: when the core wants the next, in ms; NaN once it has
   * terminated. Back early where no turn may begin until the completion ring
   * is read. */
  run(deadlineMs: f64): f64;
  /** A turn wanted at once, the next ending no sooner than `deadlineMs` from its rise, none of its
   * own where negative; nothing waited for. */
  kick(deadlineMs: f64): void;
  /** No turn begins but one `run` rings; a kick waits for `resume`. */
  pause(): void;
  /** Turns begin again as kicks ask. */
  resume(): void;
  /** The call waiting, if one does, answered here. */
  answer(): void;
  /** What the turns since asked: when the core wants the next, in ms (0 at
   * once, NaN once it has stopped), then the longest of
   * them, in ms. */
  turns(): [f64, f64];
  /** `answer` answers the core's calls from here on. */
  callOn(answer: (at: u32) => i32): void;
  /** Whether a device opened; what it is, the core reads from its runtime. */
  gpuOpen(): boolean;
}

export const XPUTE: Xpute = requireNativeModule<Xpute>("Xpute");

/** A view the core's surface `surface` draws into, 0 for none. It says when
 * the surface is bound (`onAttach`), which comes after mount, and when it is
 * let go (`onDetach`). The core sizes and draws it inside its turns. */
export interface SurfaceViewProps extends ViewProps {
  surface: u32;
  onAttach: (e: NativeSyntheticEvent<{ surface: u32 }>) => void;
  onDetach: () => void;
}

export const SurfaceView: ComponentType<SurfaceViewProps> =
  requireNativeView("Xpute");
