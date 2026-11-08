// @xpute/native

/**
 * xpute's native module, which an Expo app autolinks (ios/, android/). It
 * gives the core linked into the app, which @xpute/runtime's native platform
 * opens; the core's GPU runtime, which @xpute/gpu's `NativeGpu` grants; and
 * the view a core's surface draws into. The core is the app's core crate,
 * named by `xpute.core` in the app's package.json and built along with the
 * app (script/build-core.py).
 *
 * The core's memory is one ArrayBuffer over the region the core took, with
 * no copy. The core runs on its own thread while it has work (xpute-runtime
 * native/thread.rs). The JavaScript thread kicks it, waits on it only for a
 * turn it rings (`run`), and answers its calls, either inside `run` or when
 * it is told that a call waits (`onNotify`, `answer`).
 */

import type { f64, i32, u32 } from "@xpute/kit/abi/word.ts";
import { requireNativeModule, requireNativeView } from "expo";
import type { ComponentType } from "react";
import type { NativeSyntheticEvent, ViewProps } from "react-native";

interface Xpute {
  /** `onNotify` fires when the core has posted or asked for a turn since
   * `turns` was last called, when a call waits for `answer`, or when the core
   * has stopped. */
  addListener(event: "onNotify", listener: () => void): { remove(): void };
  mem(): ArrayBuffer;
  /** Loads the word at byte `at` of the memory with acquire. */
  load(at: u32): u32;
  /** Stores `word` at byte `at` of the memory with release. */
  store(at: u32, word: u32): void;
  /** Rings a turn that ends no sooner than `deadlineMs` after its rise, and
   * waits for it, answering its calls here meanwhile. Returns when the core
   * wants the next turn, in ms, or NaN once the core has terminated. It
   * returns early if no turn may begin until the completion ring is read. */
  run(deadlineMs: f64): f64;
  /** Asks for a turn at once and returns without waiting. The next turn ends
   * no sooner than `deadlineMs` after its rise; a negative `deadlineMs` sets
   * no deadline. */
  kick(deadlineMs: f64): void;
  /** From now on only a turn `run` rings may begin, and a kick waits for
   * `resume`. */
  pause(): void;
  /** Turns begin again as kicks ask. */
  resume(): void;
  /** Answers here the call that waits, if one does. */
  answer(): void;
  /** What the turns since the last call asked: when the core wants the next
   * turn, in ms (0 for at once, -1 for never, NaN once the core has
   * stopped), then how long the longest of those turns took, in ms. */
  turns(): [f64, f64];
  /** Makes `answer` answer the core's calls from here on. */
  callOn(answer: (at: u32) => i32): void;
  /** Whether a GPU device opened. What the device is, the core reads from
   * its GPU runtime. */
  gpuOpen(): boolean;
}

export const XPUTE: Xpute = requireNativeModule<Xpute>("Xpute");

/** A view that the core's surface `surface` draws into; 0 names no surface.
 * The view tells when the surface is bound (`onAttach`), which happens after
 * mount, and when it is let go (`onDetach`). The core sizes the surface and
 * draws it inside its turns. */
export interface SurfaceViewProps extends ViewProps {
  surface: u32;
  onAttach: (e: NativeSyntheticEvent<{ surface: u32 }>) => void;
  onDetach: () => void;
}

export const SurfaceView: ComponentType<SurfaceViewProps> =
  requireNativeView("Xpute");
