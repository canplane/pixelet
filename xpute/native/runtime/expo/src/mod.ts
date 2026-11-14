// @xpute/native-runtime

/**
 * This is xpute's native module, which an Expo app autolinks (ios/, android/).
 * It starts the runtime linked into the app beside the core, on the core's
 * own thread (ios/CXpute/xpute.h `xpute_start`), and carries the UI's link to
 * it and back, as a page's port carries it to its worker (remote.ts
 * `Remote`): the UI sends its methods on the guest's objects (`post`) and
 * answers the methods of its own objects that the runtime sent it later
 * (`answer`), and takes what the runtime sends it in a batch on one notify
 * (`onNotify`, `take`). The core's memory is one ArrayBuffer over its region,
 * with no copy (`mem`). The module tells the runtime of the app's lifecycle
 * itself (Swift, Kotlin), not through JavaScript.
 */

import type { i32, u32 } from "@xpute/kit/abi/word.ts";
import { requireNativeModule } from "expo";

/** The guest's stop, as its host reports it (xpute-native-host stop.rs
 * `Stop`): `n1` and `n2`, u64s, in their decimal digits. */
export interface Stop {
  errnum: i32;
  file: string;
  line: u32;
  n1: string;
  n2: string;
}

interface Xpute {
  /** `onNotify` fires when a batch waits for `take`, and `onTrap` when the
   * guest trapped. */
  addListener(event: "onNotify" | "onTrap", listener: () => void): { remove(): void };
  /** `onStopped` fires when the guest stopped, with its report. */
  addListener(event: "onStopped", listener: (stop: Stop) => void): { remove(): void };
  /** Starts the runtime on the core's thread, once: `named` the named handles
   * of the interfaces the UI serves (remote.ts `Remote.named`), `prefixes` the
   * prefixes the UI mounts openers at, a lowered `list<list<token>>` packet. */
  start(named: readonly u32[], prefixes: Uint8Array): void;
  /** Sends the UI's method `m` of the guest's object `h`, its request `tag`;
   * its answer comes in a batch. */
  post(tag: u32, h: u32, m: u32, pkt: Uint8Array | null): void;
  /** Answers the request `tag` the runtime sent the UI to answer later. */
  answer(tag: u32, result: i32, data: Uint8Array | null): void;
  /** Hands over the batch the UI has not taken (xpute.h `xpute_take`). */
  take(): ArrayBuffer;
  /** The core's memory, where the guest lends the UI its buffers. */
  mem(): ArrayBuffer;
  /** Writes a line to the process's output: stdout on iOS, the log under the
   * tag `xpute` on Android. */
  log(line: string): void;
}

export const XPUTE: Xpute = requireNativeModule<Xpute>("Xpute");
