// @xpute/native

/**
 * This is xpute's native module, which an Expo app autolinks (ios/, android/).
 * It gives the core linked into the app, which @xpute/runtime's native platform
 * opens. The core is the app's core crate, named by `xpute.core` in the app's
 * package.json and built along with the app (the app's script/core-build.py).
 *
 * The core's memory is one ArrayBuffer over the region the core took, with
 * no copy. The core runs on its own thread, which begins its turns at the
 * display's frames, and the runtime on another (xpute-runtime native/thread.rs,
 * serve.rs). The JavaScript thread holds the UI, which posts frames and hears a
 * batch of them when told (`onNotify`, `take`).
 */

import type { f64, i32, u32 } from "@xpute/kit/abi/word.ts";
import { requireNativeModule } from "expo";

interface Xpute {
  /** `onNotify` fires when frames wait for `take`, or a call for `answer`. */
  addListener(event: "onNotify", listener: () => void): { remove(): void };
  mem(): ArrayBuffer;
  /** Asks for a turn at the next display frame. */
  wake(): void;
  /** Says that an input came now, so the turns are interactive for a while. */
  input(): void;
  /** From now on only a turn `run` runs may begin. */
  pause(): void;
  /** Turns begin again at the display's frames. */
  resume(): void;
  /** Answers here the call that waits, if one does. */
  answer(): void;
  /** Starts the runtime's own thread; `fs` says whether the manifest asks for
   * the file device. */
  attach(fs: boolean): void;
  /** The program's numbers for frame pacing, given before the turns run. */
  pace(paceHz: f64, marginShare: f64, batchFrames: f64, settleMs: f64): void;
  /** Queues a frame for the core. */
  post(tag: u32, cmd: u32, flags: u32, result: i32, pkt: Uint8Array): void;
  /** Hands over what the UI has not heard (xpute.h `xpute_take`). */
  take(): ArrayBuffer;
  /** Completes the open of `fd`, with its bytes or a negative result. */
  fsCompleteOpen(fd: u32, result: i32, detail: u32, bytes: Uint8Array): void;
  /** Makes `answer` answer the core's calls from here on. */
  callOn(answer: (at: u32) => i32): void;
  /** Writes a line to the process's output: stdout on iOS, the log under the
   * tag `xpute` on Android. */
  say(line: string): void;
}

export const XPUTE: Xpute = requireNativeModule<Xpute>("Xpute");
