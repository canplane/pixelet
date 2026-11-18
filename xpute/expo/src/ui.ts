// @xpute/expo/ui.ts

/**
 * The UI's end of the runtime, as the web's is of its worker
 * (@xpute/runtime-web page.ts): the link's end (@xpute/link `End`) over the
 * JavaScript half of the UI's crossing, this module, on which the UI calls
 * the guest and implements its imports by the generated bindings as any end
 * does. The runtime stands in for the UI on the guest's link
 * (xpute-runtime-native remote.rs). What the UI calls goes to the module's
 * `post`, and what it answers to its `answer`; what the runtime sends comes
 * in a batch on one notify (`take`), each message a frame of the link's
 * (`read`): a method of an object the UI implements, answered back by its
 * tag, at once or later; an open of one under the handle the guest's end
 * issued, by the opener of the prefix that matched; the guest's cancel of a
 * method and its drop of an object; and the answer to a call of the UI's. A
 * buffer the guest lends is read and written through a buffer the module
 * makes over it (`bytes`). The module names the UI's handle on the
 * runtime in each of these (xpute.h `xpute_start`). The native module moves
 * no object beside the calls: the GPU's module binds a surface to its view
 * itself.
 */

import type { U8Array } from "@xpute/kit/abi/array.ts";
import { split64, type i32, type u32, type u64 } from "@xpute/kit/abi/word.ts";
import {
  ERRNO_EINPROGRESS,
  ERRNO_EMSGSIZE,
} from "@xpute/kit/status/errno.spec.ts";
import {
  type Answered,
  DROP,
  End,
  type Handle,
  type Memory,
  type Method,
  type Packet,
  pkt_of,
  ROOT,
  store_uri,
  type Tag,
} from "@xpute/link/link.ts";
import { Lift, Lower } from "@xpute/link/cabi.ts";
import { frame, HDR_SZ, read as read_frame } from "@xpute/link/frame.ts";
import { XPUTE } from "./xpute.ts";

/** The UI's end, over the module: a request posted, answered later in a
 * batch, since one thread cannot wait on the other, and an answer to a
 * request the UI owes; and the guest's lent memory, in the same
 * process. */
const ui = new (class extends End implements Memory {
  protected _out(
    tag: Tag,
    h: Handle,
    m: Method,
    res: boolean,
    result: i32,
    pkt: Packet,
  ): Answered {
    if (res) {
      XPUTE.answer(tag, result, pkt_of(pkt));
      return { result: 0, data: null };
    }
    XPUTE.post(tag, h, m, pkt_of(pkt));
    return { result: ERRNO_EINPROGRESS, data: null };
  }

  /** The `len` bytes the guest lent at `at`, its address, as `Side.bytes`
   * gives them: a buffer the module makes over those bytes alone. An empty
   * buffer has no address (link/wit/types.wit `lent`). */
  bytes(at: u64, len: u32): U8Array {
    if (len === 0) return new Uint8Array(0);
    const [lo, hi] = split64(at);
    return new Uint8Array(XPUTE.bytes(lo, hi, len));
  }
})();

/** No packet. */
const NONE = new Uint8Array(0);

/** The reader of each packet the runtime sends, and the writer of the answer
 * the UI's end gives at once, which holds no value: every method the UI
 * implements is an `async func` (xpute-runtime-native link.rs `init`), so a
 * value written there is past its room, EMSGSIZE. */
const args = new Lift(NONE);
const ret = new Lower(NONE, 0);

/** Hands the UI's end the frame the runtime sent, its packet `pkt`, and
 * answers what the end answers (`End.input`). */
function input(
  tag: Tag,
  h: Handle,
  m: Method,
  res: boolean,
  result: i32,
  pkt: U8Array | null,
): i32 {
  const p = pkt ?? NONE;
  args.reset(p.buffer, p.byteOffset, p.length);
  ret.reset(NONE.buffer, 0, 0);
  return ui.input(tag, h, m, res, result, args, ret);
}

/** The frame `read` reads each of a batch into. */
const f = frame();

/** Hears each frame of `batch` (xpute-runtime-native remote.rs), handed to
 * the UI's end: its words, then its packet, padded to 8 bytes. A method of
 * an object's is owed an answer by its tag, so its result at once goes back
 * as a later answer; the root's methods and a drop are the link's own and
 * owed none. */
function read(batch: ArrayBuffer): void {
  const view = new DataView(batch);
  for (let at = 0; at < batch.byteLength;) {
    read_frame(view, at, f);
    const pkt =
      f.pkt_len === 0 ? null : new Uint8Array(batch, at + HDR_SZ, f.pkt_len);
    at += (HDR_SZ + f.pkt_len + 7) & ~7;
    const { tag, handle: h, method: m, res } = f;
    const result = input(tag, h, m, res, f.result, pkt);
    if (res || h === ROOT || m === DROP || result === ERRNO_EINPROGRESS)
      continue;
    ui.task_return(tag, ret.finish() < 0 ? ERRNO_EMSGSIZE : result);
  }
}

/** Opens the UI's end of the runtime. */
export function open_ui(): End & Memory {
  XPUTE.addListener("onNotify", () => read(XPUTE.take()));
  return ui;
}

/** Starts the runtime on the guest's thread with what the UI implements, once
 * it is mounted on the UI's end: its named handles, and its prefixes, each
 * with its interface's named handle, lowered as a
 * `list<tuple<u32, list<token>>>`, the fixed part at 0 (xpute.h
 * `xpute_start`). */
export function start(): void {
  const prefixes = ui.prefixes();
  const pkt = pkt_of([
    8,
    (w) => {
      const b = w.list(0, prefixes.length, 12, 4);
      if (b === null) return;
      prefixes.forEach(([h, p], k) => {
        w.u32(b + k * 12, h);
        store_uri(w, b + k * 12 + 4, p);
      });
    },
  ]);
  XPUTE.start(ui.named(), pkt!);
}
