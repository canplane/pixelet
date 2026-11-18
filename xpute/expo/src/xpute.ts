// @xpute/expo/xpute.ts

/**
 * xpute's native module, which an Expo app autolinks (ios/, android/), by its
 * native functions and events. It starts the runtime linked into the app
 * beside the guest, on the guest's own thread (ios/CXpute/xpute.h
 * `xpute_start`), and carries the UI's calls to it and back (ui.ts). The
 * module tells the runtime of the app's lifecycle itself (Swift, Kotlin), not
 * through JavaScript.
 */

import type { i32, u32 } from "@xpute/kit/abi/word.ts";
import type { UiStop } from "@xpute/host/runtime.ts";
import type { Level } from "@xpute/host/abi/log/log.enum.ts";
import { requireNativeModule } from "expo";

/** The guest's stop, as its host reports it (xpute-runtime-native listener.rs
 * `Stop`): `n1` and `n2`, u64s, in their decimal digits. */
interface Report {
  errnum: i32;
  file: string;
  line: u32;
  n1: string;
  n2: string;
}

/** The stop `r` reports, as the UI hears it (@xpute/host `UiStop`). */
function stop_of(r: Report): UiStop {
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
  /** Starts the runtime on the guest's thread and attaches the UI to it
   * (xpute.h `xpute_start`): `named` the named handles of the interfaces the
   * UI implements, `prefixes` the prefixes the UI mounts openers at, in their
   * order, each with its interface's named handle, a lowered
   * `list<tuple<u32, list<token>>>` packet. The module keeps the UI's handle
   * it answers, which each of the calls below names. */
  start(named: readonly u32[], prefixes: Uint8Array): void;
  /** Sends the UI's method `m` of the guest's object `h`, its request `tag`;
   * its answer comes in a batch. */
  post(tag: u32, h: u32, m: u32, pkt: Uint8Array | null): void;
  /** Answers the request `tag` the runtime sent the UI to answer later. */
  answer(tag: u32, result: i32, data: Uint8Array | null): void;
  /** Hands over the batch the UI has not taken (xpute.h `xpute_take`). */
  take(): ArrayBuffer;
  /** The `len` bytes the guest lent at the address whose low and high words
   * are `lo` and `hi`, a buffer over them, the UI's until the lend's answer. */
  bytes(lo: u32, hi: u32, len: u32): ArrayBuffer;
  /** Writes a log record by the runtime's writer (`log`). */
  log(level: Level, context: string, msg: string): void;
  /** The guest's last stop, or null before the guest stopped (xpute.h
   * `xpute_stop`). */
  stop(): Report | null;
  /** What trapped the guest, or null before it trapped (xpute.h
   * `xpute_trap`). */
  trap(): string | null;
}

export const XPUTE: Xpute = requireNativeModule<Xpute>("Xpute");

/** Adds `hear` to what is told the guest's stop, as the link has it, over the
 * module's `onStopped`, which xpute.h `xpute_stopped_on` sends, and tells it
 * the stop the runtime keeps, which a UI started after the stop has not
 * heard. A stop told between the two is the kept one, and is heard again. */
export function stopped_on(hear: (stop: UiStop) => void): { remove(): void } {
  const sub = XPUTE.addListener("onStopped", (r) => hear(stop_of(r)));
  const r = XPUTE.stop();
  if (r !== null) hear(stop_of(r));
  return sub;
}

/** Adds `hear` to what is told that the guest trapped, with what stopped it,
 * as the web's trap carries its error, over the module's `onTrap`, which
 * xpute.h `xpute_trap_on` sends, and tells it the trap the runtime keeps, as
 * `stopped_on` tells the stop. */
export function trap_on(hear: (err: string) => void): { remove(): void } {
  const sub = XPUTE.addListener("onTrap", (r) => hear(r.err));
  const err = XPUTE.trap();
  if (err !== null) hear(err);
  return sub;
}

/** Writes `msg` at `level`, about `context`, by the runtime's writer,
 * which writes the host's records (xpute-runtime-native log.rs). */
export function log(level: Level, context: string, msg: string): void {
  XPUTE.log(level, context, msg);
}
