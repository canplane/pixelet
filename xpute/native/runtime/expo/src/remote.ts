// @xpute/native-runtime/remote.ts

/**
 * The UI's end of the runtime's link, as a page's is of its worker's
 * (@xpute/web-runtime page.ts): a `Remote` over this module. What the UI
 * sends goes to the module's `post` and `answer`, and what the runtime sends
 * comes in a batch on one notify (`take`), each message an XTP packet read
 * into what a port carries (xpute-native-runtime remote.rs). A buffer the
 * guest lends is read and written through a buffer the module makes over its
 * span (`bytes`).
 */

import type { U8Array } from "@xpute/kit/abi/array.ts";
import { split64, type i32, type u32, type u64 } from "@xpute/kit/abi/word.ts";
import { bug } from "@xpute/kit/status/bug.ts";
import { ERRNO_EBADMSG, ERRNO_ENOSYS } from "@xpute/kit/status/errno.spec.ts";
import { type BranchCursor, TreeReader } from "@xpute/kit/wire/xtp/mod.ts";
import {
  Lift,
  load_uri,
  packet,
  type Port,
  Remote,
  store_uri,
} from "@xpute/link";
import type { ToLink, ToUi } from "@xpute/link/remote.ts";
import { XPUTE } from "./xpute.ts";

/** Each message of a batch by its kind, its first field, read into what a
 * port carries to the Remote. A method told (`later` 0) has no tag. */
const TO_UI: Readonly<Record<u32, (f: BranchCursor) => ToUi>> = {
  0: (f) => ({
    tag: f.get<u32>(1),
    result: f.get<i32>(2),
    data: f.get<U8Array | null>(3),
  }),
  2: (f) => ({
    h: f.get<u32>(1),
    m: f.get<u32>(2),
    tag: f.get<u32>(3) === 1 ? f.get<u32>(4) : null,
    pkt: f.get<U8Array | null>(5),
  }),
  3: (f) => ({
    open: f.get<u32>(1),
    uri: load_uri(new Lift(f.get<U8Array>(2)), 0),
  }),
  4: (f) => ({ cancel: f.get<u32>(1) }),
  5: (f) => ({ drop: f.get<u32>(1) }),
};

/** A message's header: its packet's length, a u32, and four bytes more, so
 * that the packet starts on a word. Each message is padded to 8 bytes. */
const HDR_SZ = 8;

/** Hears each message of `batch`. */
function read(batch: ArrayBuffer, hear: (m: ToUi) => void): void {
  const view = new DataView(batch);
  for (let at = 0; at < batch.byteLength;) {
    const len = view.getUint32(at, true);
    const f = new TreeReader(
      new Uint8Array(batch, at + HDR_SZ, len),
    ).read_branch();
    hear((TO_UI[f.get<u32>(0)] ?? bug(ERRNO_EBADMSG))(f));
    at += (HDR_SZ + len + 7) & ~7;
  }
}

/** Opens the UI's end of the runtime's link. A thing given beside the link
 * has no device form: the GPU's module binds a surface to its view itself
 * (@xpute/gpu-native-runtime `SurfaceView`). */
export function open_remote(): Remote {
  const port: Port = {
    postMessage(message) {
      const m = message as ToLink;
      if ("give" in m) bug(ERRNO_ENOSYS);
      if ("answer" in m) return XPUTE.answer(m.answer, m.result, m.data);
      XPUTE.post(m.tag, m.h, m.m, m.pkt);
    },
    onmessage: null,
  };
  XPUTE.addListener("onNotify", () =>
    read(XPUTE.take(), (m) => port.onmessage?.({ data: m })),
  );
  return new Remote(port);
}

/** Starts the runtime on the guest's thread with what the UI serves, once it
 * is mounted on `remote`: its named handles, and its prefixes, each with its
 * interface's named handle, lowered as a `list<tuple<u32, list<token>>>`,
 * the fixed part at 0 (xpute.h `xpute_start`). */
export function start(remote: Remote): void {
  const prefixes = remote.prefixes;
  const pkt = packet([
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
  XPUTE.start(remote.named, pkt!);
}

/** The `len` bytes the guest lent at `at`, its address, as Side `bytes`
 * gives them: a buffer the module makes over that span alone. An empty span
 * has no address (link/wit/types.wit `lent`). */
export function bytes(at: u64, len: u32): U8Array {
  if (len === 0) return new Uint8Array(0);
  const [lo, hi] = split64(at);
  return new Uint8Array(XPUTE.bytes(lo, hi, len));
}
