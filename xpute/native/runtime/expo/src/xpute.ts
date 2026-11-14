// @xpute/native-runtime/xpute.ts

/** The module's native functions and events (ios/, android/). */

import type { i32, u32 } from "@xpute/kit/abi/word.ts";
import type { Stop } from "@xpute/link";
import { requireNativeModule } from "expo";

/** The guest's stop, as its host reports it (xpute-native-runtime listener.rs
 * `Stop`): `n1` and `n2`, u64s, in their decimal digits. */
interface Report {
  errnum: i32;
  file: string;
  line: u32;
  n1: string;
  n2: string;
}

/** The stop `r` reports, as the link has it (@xpute/link `Stop`). */
function stop_of(r: Report): Stop {
  return { ...r, n1: BigInt(r.n1), n2: BigInt(r.n2) };
}

interface Xpute {
  /** `onNotify` fires when a batch waits for `take`. */
  addListener(event: "onNotify", listener: () => void): { remove(): void };
  /** `onTrap` fires when the guest trapped, with what stopped it. */
  addListener(
    event: "onTrap",
    listener: (r: { err: string }) => void,
  ): { remove(): void };
  /** `onStopped` fires when the guest stopped, with its report. */
  addListener(
    event: "onStopped",
    listener: (r: Report) => void,
  ): { remove(): void };
  /** Starts the runtime on the guest's thread (xpute.h `xpute_start`):
   * `named` the named handles of the interfaces the UI serves (@xpute/link remote.ts
   * `Remote.named`), `prefixes` the prefixes the UI mounts openers at, each
   * with its interface's named handle, a lowered
   * `list<tuple<u32, list<token>>>` packet. */
  start(named: readonly u32[], prefixes: Uint8Array): void;
  /** Sends the UI's method `m` of the guest's object `h`, its request `tag`;
   * its answer comes in a batch. */
  post(tag: u32, h: u32, m: u32, pkt: Uint8Array | null): void;
  /** Answers the request `tag` the runtime sent the UI to answer later. */
  answer(tag: u32, result: i32, data: Uint8Array | null): void;
  /** Hands over the batch the UI has not taken (xpute.h `xpute_take`). */
  take(): ArrayBuffer;
  /** The guest's memory, where it lends the UI its buffers. */
  mem(): ArrayBuffer;
  /** Writes a line to the process's output (`log`). */
  log(line: string): void;
}

export const XPUTE: Xpute = requireNativeModule<Xpute>("Xpute");

/** Adds `hear` to what is told the guest's stop, as the link has it, over the
 * module's `onStopped`, which xpute.h `xpute_stopped_on` sends. */
export function stopped_on(hear: (stop: Stop) => void): { remove(): void } {
  return XPUTE.addListener("onStopped", (r) => hear(stop_of(r)));
}

/** Adds `hear` to what is told that the guest trapped, with what stopped it,
 * as a page's trap carries its error, over the module's `onTrap`, which
 * xpute.h `xpute_trap_on` sends. */
export function trap_on(hear: (err: string) => void): { remove(): void } {
  return XPUTE.addListener("onTrap", (r) => hear(r.err));
}

/** Writes `line` to the process's output: stdout on iOS, the log under the
 * tag `xpute` on Android. */
export function log(line: string): void {
  XPUTE.log(line);
}
