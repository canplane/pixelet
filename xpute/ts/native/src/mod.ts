// @xpute/native

/**
 * This is xpute's native module, which an Expo app autolinks (ios/, android/).
 * It gives the core linked into the app, which @xpute/runtime's native platform
 * opens; the core's GPU runtime, which @xpute/gpu's `NativeGpu` grants; and the
 * view a core's surface draws into. The core is the app's core crate, named by
 * `xpute.core` in the app's package.json and built along with the app
 * (script/core-build.py).
 *
 * The core's memory is one ArrayBuffer over the region the core took, with
 * no copy. The core runs on its own thread, which begins its turns at the
 * display's frames, and the runtime on another (xpute-runtime native/thread.rs,
 * serve.rs). The JavaScript thread holds the UI, which posts frames and hears a
 * batch of them when told (`onNotify`, `take`).
 */

import type { f64, i32, u32 } from "@xpute/kit/abi/word.ts";
import { requireNativeModule, requireNativeView } from "expo";
import type { ComponentType } from "react";
import type { NativeSyntheticEvent, ViewProps } from "react-native";

interface Xpute {
  /** `onNotify` fires when frames wait for `take`, or a call for `answer`. */
  addListener(event: "onNotify", listener: () => void): { remove(): void };
  mem(): ArrayBuffer;
  /** Loads the word at byte `at` of the memory with acquire. */
  load(at: u32): u32;
  /** Stores `word` at byte `at` of the memory with release. */
  store(at: u32, word: u32): void;
  /** Asks for a turn at the next display frame. */
  wake(): void;
  /** Says that an input came now, so the turns are interactive for a while. */
  input(): void;
  /** From now on only a turn `run` rings may begin. */
  pause(): void;
  /** Turns begin again at the display's frames. */
  resume(): void;
  /** Answers here the call that waits, if one does. */
  answer(): void;
  /** Starts the runtime's own thread over the rings at these offsets; `fs`
   * says whether the machine asks for the file device. */
  attach(sqAt: u32, cqAt: u32, fs: boolean): void;
  /** The program's numbers for frame pacing, given before the turns run. */
  pace(paceHz: f64, marginShare: f64, batchFrames: f64, settleMs: f64): void;
  /** Queues a frame for the submission ring. */
  post(tag: u32, cmd: u32, flags: u32, result: i32, pkt: Uint8Array): void;
  /** Hands over what the UI has not heard (xpute.h `xpute_take`). */
  take(): ArrayBuffer;
  /** Completes the open of `fd`, with its bytes or a negative result. */
  fsOpened(fd: u32, result: i32, detail: u32, bytes: Uint8Array): void;
  /** Completes the close of `fd`. */
  fsClosed(fd: u32, result: i32): void;
  /** Calls the core: submits `cmd` and rings turns until its reply, which it
   * returns, answering the turns' calls here meanwhile. */
  call(cmd: u32, pkt: Uint8Array): i32;
  /** Makes `answer` answer the core's calls from here on. */
  callOn(answer: (at: u32) => i32): void;
  /** This says whether a GPU device opened. What the device is, the core reads
   * from its GPU runtime. */
  gpuOpen(): boolean;
  /** Writes a line to the process's output: stdout on iOS, the log under the
   * tag `xpute` on Android. */
  say(line: string): void;
}

export const XPUTE: Xpute = requireNativeModule<Xpute>("Xpute");

/** The core's surface `surface` draws into this view, and 0 names no surface.
 * The view tells when the surface is bound (`onAttach`), which happens after
 * mount, and when it is let go (`onDetach`). The core sizes the surface and
 * draws it inside its turns. */
export interface SurfaceViewProps extends ViewProps {
  surface: u32;
  /** Whether the view tells the core its touches itself. */
  touches?: boolean;
  onAttach: (e: NativeSyntheticEvent<{ surface: u32 }>) => void;
  onDetach: () => void;
}

export const SurfaceView: ComponentType<SurfaceViewProps> =
  requireNativeView("Xpute");
