// @xpute/link/link.test.ts

import { test } from "vitest";
import assert from "node:assert/strict";

import {
  ERRNO_EBADMSG,
  ERRNO_EPIPE,
  ERRNO_EINPROGRESS,
  ERRNO_EMSGSIZE,
  ERRNO_ENOENT,
} from "@xpute/kit/status/errno.spec.ts";
import type { U8Array } from "@xpute/kit/abi/array.ts";
import { XputeError } from "@xpute/kit/status/error.ts";
import { type Frame, frame, HDR_SZ, lay, read } from "./frame.ts";
import { LAYOUT, OPEN_AT } from "./frame.spec.ts";
import {
  answer,
  CANCEL,
  NAMED,
  pkt_of,
  ROOT,
  Side,
  store_err,
  store_uri,
  type Tag,
  uri,
} from "./link.ts";
import { Table } from "./resource.ts";

const INBOX = 4 * LAYOUT.WORDS;
const CAP = 2048;
const OUTBOX = INBOX + CAP;

/** The error ENOENT made at line 7 of `a/b.rs`, with the message `msg`. */
const made = (msg: string | null) =>
  new XputeError(ERRNO_ENOENT, msg, ["a/b.rs", 7]);

/** An interface the host implements: method 0 answers at once with a word,
 * method 1 later, method 2 at once with a value past the cap, and methods 3
 * and 4 fail at once, with a message and with none. */
const INTERFACE = (NAMED | 1) >>> 0;

/** The host's side over a guest that only lays its link and keeps each
 * frame the host lays in its inbox; and the tag method 1 left to answer. */
function link(): {
  side: Side;
  view: DataView;
  frames: Frame[];
  later: () => Tag;
} {
  const mem = new ArrayBuffer(INBOX + 2 * CAP);
  const view = new DataView(mem);
  view.setUint32(4 * LAYOUT.INBOX_AT, INBOX, true);
  view.setUint32(4 * LAYOUT.CAP_AT, CAP, true);
  const frames: Frame[] = [];
  let later: Tag | null = null;
  const side = new Side({
    mem,
    in: () => {
      frames.push(read(view, INBOX, frame()));
      return 0;
    },
  });
  side.mount_named(INTERFACE, {
    call(m, tag, _args, ret) {
      if (m === 1) {
        later = tag;
        return ERRNO_EINPROGRESS;
      }
      if (m === 3) return store_err(ret, made("no café"));
      if (m === 4) return store_err(ret, made(null));
      ret.start(m === 0 ? 4 : CAP);
      if (m === 0) ret.u32(0, 7);
      return 0;
    },
    drop() {},
  });
  side.set_layout(0n);
  return { side, view, frames, later: () => later! };
}

test("the answer a side lays in the frame its request came in is marked RES and carries the request's tag", () => {
  const { side, view } = link();
  const out = (at: bigint) => side.recv(at);
  for (const [tag, method, result] of [
    [5, 0, 0],
    [6, 2, ERRNO_EMSGSIZE],
  ] as const) {
    lay(view, OUTBOX, {
      tag,
      res: false,
      handle: INTERFACE,
      method,
      result: 0,
      pkt_len: 0,
    });
    assert.equal(out(BigInt(OUTBOX)), result);
    const f = read(view, OUTBOX, frame());
    assert.deepEqual(
      [f.res, f.tag, f.handle, f.method, f.result],
      [true, tag, INTERFACE, method, result],
    );
  }
});

test("a later answer is marked RES and carries the request's tag", () => {
  const { side, view, frames, later } = link();
  const out = (at: bigint) => side.recv(at);
  for (const [tag, sz, result] of [
    [5, 4, 0],
    [6, CAP, ERRNO_EMSGSIZE],
  ] as const) {
    lay(view, OUTBOX, {
      tag,
      res: false,
      handle: INTERFACE,
      method: 1,
      result: 0,
      pkt_len: 0,
    });
    assert.equal(out(BigInt(OUTBOX)), ERRNO_EINPROGRESS);
    side.task_return(later(), 0, [sz, (w) => w.u32(0, 9)]);
    const f = frames.pop()!;
    assert.deepEqual([f.res, f.tag, f.result], [true, tag, result]);
  }
});

test("an error made at a place with a message arrives with both, and one made with no message arrives with none", () => {
  const { side, view, frames, later } = link();
  const mem = new Uint8Array(view.buffer);
  /** The error the answer laid at `at` is, as its caller is thrown it. */
  const arrived = (at: number): XputeError => {
    const f = read(view, at, frame());
    const data =
      f.pkt_len === 0 ? null : mem.slice(at + HDR_SZ, at + HDR_SZ + f.pkt_len);
    try {
      answer({ result: f.result, data });
    } catch (e) {
      if (e instanceof XputeError) return e;
    }
    throw new Error("no error");
  };
  const request = (tag: Tag, method: number) => {
    lay(view, OUTBOX, {
      tag,
      res: false,
      handle: INTERFACE,
      method,
      result: 0,
      pkt_len: 0,
    });
    return side.recv(BigInt(OUTBOX));
  };
  const facts = (e: XputeError) => [e.errnum, e.file, e.line, e.msg];
  for (const [method, msg] of [
    [3, "no café"],
    [4, null],
  ] as const) {
    assert.equal(request(5, method), ERRNO_ENOENT);
    assert.deepEqual(facts(arrived(OUTBOX)), [ERRNO_ENOENT, "a/b.rs", 7, msg]);
  }
  for (const msg of ["no café", null]) {
    assert.equal(request(7, 1), ERRNO_EINPROGRESS);
    side.task_err(later(), made(msg));
    assert.equal(frames.pop()!.res, true);
    assert.deepEqual(facts(arrived(INBOX)), [ERRNO_ENOENT, "a/b.rs", 7, msg]);
  }
  // A packet that holds no error's is EBADMSG, and a trapped link's EPIPE,
  // which no frame carried, is made where it is read.
  const read_of = (result: number, data: U8Array | null): XputeError => {
    try {
      answer({ result, data });
    } catch (e) {
      if (e instanceof XputeError) return e;
    }
    throw new Error("no error");
  };
  const bad = read_of(ERRNO_ENOENT, new Uint8Array([0x0c, 0, 0, 0]));
  assert.deepEqual([bad.errnum, bad.msg], [ERRNO_EBADMSG, null]);
  const bare = read_of(ERRNO_EPIPE, null);
  assert.equal(bare.errnum, ERRNO_EPIPE);
  assert.ok(bare.file.endsWith("link.ts"), bare.file);

  // A refusal of the link's own answers in a frame as any error does: a
  // method of a named handle nothing is mounted under is ENOENT, made where
  // the link refused it.
  lay(view, OUTBOX, {
    tag: 9,
    res: false,
    handle: (NAMED | 2) >>> 0,
    method: 0,
    result: 0,
    pkt_len: 0,
  });
  assert.equal(side.recv(BigInt(OUTBOX)), ERRNO_ENOENT);
  const f = read(view, OUTBOX, frame());
  assert.deepEqual([f.res, f.result], [true, ERRNO_ENOENT]);
  const refused = arrived(OUTBOX);
  assert.deepEqual([refused.errnum, refused.msg], [ERRNO_ENOENT, null]);
  assert.ok(refused.file.endsWith("link.ts"), refused.file);
  assert.ok(refused.line > 0);
});

test("a cancel runs the stop its request put once, and none once the request has answered", () => {
  const { side, view, later } = link();
  const out = (at: bigint) => side.recv(at);
  const send = (tag: Tag, handle: number, method: number, arg: Tag | null) => {
    if (arg !== null) view.setUint32(OUTBOX + HDR_SZ, arg, true);
    lay(view, OUTBOX, {
      tag,
      res: false,
      handle,
      method,
      result: 0,
      pkt_len: arg === null ? 0 : 4,
    });
    return out(BigInt(OUTBOX));
  };
  let stops = 0;
  assert.equal(send(5, INTERFACE, 1, null), ERRNO_EINPROGRESS);
  side.on_cancel(later(), () => stops++);
  send(6, ROOT, CANCEL, 5);
  send(7, ROOT, CANCEL, 5);
  assert.equal(stops, 1);

  assert.equal(send(8, INTERFACE, 1, null), ERRNO_EINPROGRESS);
  side.on_cancel(later(), () => stops++);
  side.task_return(later(), 0);
  send(9, ROOT, CANCEL, 8);
  assert.equal(stops, 1);
});

test("the root's OPEN_AT opens an object under the handle the sender issued, by the opener at its place", () => {
  const { side, view } = link();
  const opened: [string, number][] = [];
  side.mount(INTERFACE, uri("none"), () => ({ call: () => -1, drop() {} }));
  side.mount(INTERFACE, uri("obj"), (u, h) => {
    opened.push([u.map((t) => t.val).join("/"), h]);
    return { call: (m) => m + 40, drop() {} };
  });
  const h = 9;
  const pkt = pkt_of([
    16,
    (w) => {
      w.u32(0, h);
      w.u32(4, 1);
      store_uri(w, 8, uri("obj", "a"));
    },
  ])!;
  new Uint8Array(view.buffer).set(pkt, OUTBOX + HDR_SZ);
  const send = (handle: number, method: number, pkt_len: number) => {
    lay(view, OUTBOX, {
      tag: 0,
      res: false,
      handle,
      method,
      result: 0,
      pkt_len,
    });
    return side.recv(BigInt(OUTBOX));
  };
  assert.equal(send(ROOT, OPEN_AT, pkt.length), 0);
  assert.deepEqual(opened, [["obj/a", h]]);
  assert.equal(send(h, 2, 0), 42);
});

test("a table holds an object under a handle the other end issued, as under its own, and its slot follows that end's next generation", () => {
  const issuer = new Table<number>();
  const holder = new Table<number>();
  const a = issuer.add(1);
  const b = issuer.add(2);
  holder.put(b, 2);
  assert.equal(holder.get(b), 2);
  assert.equal(holder.get(a), undefined);
  assert.equal(holder.remove(b), 2);
  assert.equal(issuer.remove(b), 2);
  const c = issuer.add(3);
  holder.put(c, 3);
  holder.put(a, 1);
  assert.equal(holder.get(b), undefined);
  assert.equal(holder.get(c), 3);
  assert.equal(holder.get(a), 1);
});
