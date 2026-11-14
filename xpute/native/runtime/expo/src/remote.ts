// @xpute/native-runtime/remote.ts

/**
 * The UI's end of the runtime's link, as a page's is of its worker's
 * (@xpute/web-runtime page.ts): a `Remote` over this module. What the UI
 * sends goes to the module's `post` and `answer`, and what the runtime sends
 * comes in a batch on one notify (`take`), each message an XTP packet read
 * into what a port carries (xpute-native-runtime remote.rs). The buffers the
 * guest lends lie in its memory, which the module hands over whole (`mem`),
 * read here by range (`bytes`).
 */

import type { U8Array } from "@xpute/kit/abi/array.ts";
import type { i32, u32 } from "@xpute/kit/abi/word.ts";
import { bug } from "@xpute/kit/status/bug.ts";
import { ERRNO_EBADMSG, ERRNO_ENOSYS } from "@xpute/kit/status/errno.spec.ts";
import { type BranchCursor, TreeReader } from "@xpute/kit/wire/xtp/mod.ts";
import {
  Lift,
  load_uri,
  packet,
  type Port,
  Remote,
  type Stop,
  store_uri,
} from "@xpute/link";
import type { ToLink, ToUi } from "@xpute/link/remote.ts";
import { stopped_on, trap_on, XPUTE } from "./xpute.ts";

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

/** What hears the guest's end: its stop, with the report, and its trap, with
 * what stopped it. */
export interface Listener {
  stopped: (stop: Stop) => void;
  trapped: (err: string) => void;
}

/** Starts the runtime on the guest's thread with what the UI serves, once it
 * is mounted on `remote`: its named handles, and its prefixes, each with its
 * interface's named handle, lowered as a `list<tuple<u32, list<token>>>`,
 * the fixed part at 0 (xpute.h `xpute_start`). `listener` hears the guest's
 * stop and trap from before the start, so that one the runtime tells again
 * at the start, from before a reload, reaches them. */
export function start(remote: Remote, listener: Listener): void {
  stopped_on(listener.stopped);
  trap_on(listener.trapped);
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

/** The `len` bytes at `at` in the guest's memory, as Side `bytes` gives
 * them. The memory is asked for each time, as the runtime sets it up once it
 * has started. */
export function bytes(at: u32, len: u32): U8Array {
  return new Uint8Array(XPUTE.mem(), at, len);
}
