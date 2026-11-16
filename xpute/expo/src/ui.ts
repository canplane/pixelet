// @xpute/expo/ui.ts

/**
 * The UI's end of the runtime, as a page's is of its worker
 * (@xpute/runtime-web page.ts): the link's end (@xpute/link `End`) over the
 * JavaScript half of the UI's crossing, this module, on which the UI calls
 * the guest and implements its imports by the generated bindings as any end
 * does. The runtime stands in for the UI on the guest's link
 * (xpute-runtime-native remote.rs). What the UI calls goes to the module's
 * `post`, and what it answers to its `answer`; what the runtime sends comes
 * in a batch on one notify (`take`), each message an XTP packet by its kind
 * (`HEAR`): a method of an object the UI implements, answered back by its
 * tag, at once or later; an open of one under the handle the guest's end
 * issued, by the opener of the prefix that matched; the guest's cancel of a
 * method and its drop of an object; and the answer to a call of the UI's. A
 * buffer the guest lends is read and written through a buffer the module
 * makes over its span (`bytes`). The module names the UI's handle on the
 * runtime in each of these (xpute.h `xpute_start`). A device's module moves
 * no object beside the calls: the GPU's module binds a surface to its view
 * itself.
 */

import type { U8Array } from "@xpute/kit/abi/array.ts";
import { split64, type i32, type u32, type u64 } from "@xpute/kit/abi/word.ts";
import { bug } from "@xpute/kit/status/bug.ts";
import {
  ERRNO_EBADMSG,
  ERRNO_EINPROGRESS,
} from "@xpute/kit/status/errno.spec.ts";
import { type BranchCursor, TreeReader } from "@xpute/kit/wire/xtp/mod.ts";
import {
  type Answered,
  CANCEL,
  DROP,
  End,
  type Handle,
  Lift,
  load_uri,
  type Memory,
  type Method,
  type Packet,
  pkt_of,
  ROOT,
  store_uri,
  type Tag,
} from "@xpute/link";
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
   * gives them: a buffer the module makes over that span alone. An empty
   * span has no address (link/wit/types.wit `lent`). */
  bytes(at: u64, len: u32): U8Array {
    if (len === 0) return new Uint8Array(0);
    const [lo, hi] = split64(at);
    return new Uint8Array(XPUTE.bytes(lo, hi, len));
  }
})();

/** Each message of a batch by its kind, its first field
 * (xpute-runtime-native remote.rs), handed to the UI's end: a method's reply
 * at once goes back as a later answer, and the guest's cancel and drop are
 * the link's own. */
const HEAR: Readonly<Record<u32, (f: BranchCursor) => void>> = {
  0: (f) =>
    void ui.input(
      f.get<u32>(1),
      ROOT,
      0,
      true,
      f.get<i32>(2),
      f.get<U8Array | null>(3),
    ),
  2: (f) => {
    const tag = f.get<u32>(3);
    const reply = ui.input(
      tag,
      f.get<u32>(1),
      f.get<u32>(2),
      false,
      0,
      f.get<U8Array | null>(4),
    );
    if (reply === ERRNO_EINPROGRESS) return;
    const { result, data } =
      typeof reply === "number" ? { result: reply, data: null } : reply;
    ui.task_return(tag, result, data);
  },
  3: (f) =>
    ui.open_at(
      f.get<u32>(2),
      f.get<u32>(1),
      load_uri(new Lift(f.get<U8Array>(3)), 0),
    ),
  4: (f) => {
    const tag = f.get<u32>(1);
    ui.input(0, ROOT, CANCEL, false, 0, pkt_of([4, (w) => w.u32(0, tag)]));
  },
  5: (f) => void ui.input(0, f.get<u32>(1), DROP, false, 0, null),
};

/** A message's header: its packet's length, a u32, and four bytes more, so
 * that the packet starts on a word. Each message is padded to 8 bytes. */
const HDR_SZ = 8;

/** Hears each message of `batch`. */
function read(batch: ArrayBuffer): void {
  const view = new DataView(batch);
  for (let at = 0; at < batch.byteLength;) {
    const len = view.getUint32(at, true);
    const f = new TreeReader(
      new Uint8Array(batch, at + HDR_SZ, len),
    ).read_branch();
    (HEAR[f.get<u32>(0)] ?? bug(ERRNO_EBADMSG))(f);
    at += (HDR_SZ + len + 7) & ~7;
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
