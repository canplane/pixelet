// @xpute/link/link.test.ts

import { test } from "vitest";
import assert from "node:assert/strict";

import {
  ERRNO_EINPROGRESS,
  ERRNO_EMSGSIZE,
} from "@xpute/kit/status/errno.spec.ts";
import { type Frame, HDR_SZ, lay, read } from "./frame.ts";
import { LAYOUT } from "./frame.spec.ts";
import { CANCEL, NAMED, ROOT, Side, type Tag } from "./link.ts";
import { Table } from "./resource.ts";

const INBOX = 4 * LAYOUT.WORDS;
const CAP = 64;
const OUTBOX = INBOX + CAP;

/** An interface the host implements: method 0 answers at once with a word,
 * method 1 later, and method 2 at once with a value past the cap. */
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
      frames.push(read(view, INBOX));
      return 0;
    },
  });
  side.mount_named(INTERFACE, {
    call(m, tag) {
      if (m === 0) return { result: 0, data: [4, (w) => w.u32(0, 7)] };
      if (m === 1) {
        later = tag;
        return ERRNO_EINPROGRESS;
      }
      return { result: 0, data: [CAP, () => {}] };
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
    const f = read(view, OUTBOX);
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
