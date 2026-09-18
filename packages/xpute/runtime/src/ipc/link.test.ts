// @xpute/runtime/ipc/link.test.ts

import { assert, assertEquals } from "@std/assert";

import { Errno } from "@xpute/core/errno.ts";
import { XputeError } from "@xpute/core/error.ts";

import { Frame, FrameFlag, type FrameMessage, make_tx_id, tx_seq, tx_worker_id } from "./frame.ts";
import { CommandPort } from "./port.ts";
import { serve, virtual_link_pair } from "./link.ts";

const ECHO = 7;
const FAIL = 8;
const THROW = 9;

function served(): { client: ReturnType<typeof virtual_link_pair>[0]; replies: FrameMessage[] } {
  const [client, server] = virtual_link_pair();
  const port = new CommandPort();
  port.on(ECHO, (frame) => {
    const args = new Uint32Array(frame.buf, frame.args_offset(), 1);
    const n = args[0];
    frame.lay_out(4, 0);
    args[0] = n + 1;
    return null;
  });
  port.on(FAIL, () => {
    throw new XputeError(Errno.EBUSY);
  });
  port.on(THROW, () => {
    throw new Error("not an XputeError");
  });
  serve(server, port);
  const replies: FrameMessage[] = [];
  client.on((msg) => replies.push(msg));
  return { client, replies };
}

function request(tx: number, cmd: number, flags: number, arg = 0): ArrayBuffer {
  const buf = new ArrayBuffer(64);
  const frame = new Frame(buf);
  frame.begin(tx, cmd, flags);
  frame.lay_out(4, 0);
  new Uint32Array(buf, frame.args_offset(), 1)[0] = arg;
  return buf;
}

const settle = () => new Promise((r) => setTimeout(r, 0));

Deno.test("tx_id - the worker id rides in the high bits and the sequence in the low", () => {
  const tx = make_tx_id(3, 0x123456);
  assertEquals(tx_worker_id(tx), 3);
  assertEquals(tx_seq(tx), 0x123456);
  assertEquals(tx_seq(make_tx_id(1, 0x1000000 + 5)), 5);
});

Deno.test("virtual link - sending transfers: the sender's buffer is detached", async () => {
  const { client, replies } = served();
  const buf = request(make_tx_id(1, 1), ECHO, FrameFlag.ACKREQ, 41);
  client.send({ frame: buf, spill: null });
  assertEquals(buf.byteLength, 0);
  await settle();
  assertEquals(replies.length, 1);
});

Deno.test("serve - ACKREQ gets exactly one reply, with RES set by the runtime and the handler's result", async () => {
  const { client, replies } = served();
  const tx = make_tx_id(2, 9);
  client.send({ frame: request(tx, ECHO, FrameFlag.ACKREQ, 41), spill: null });
  await settle();
  assertEquals(replies.length, 1);
  const reply = new Frame(replies[0].frame);
  assertEquals(reply.tx_id, tx);
  assertEquals(reply.flags, FrameFlag.ACKREQ | FrameFlag.RES);
  assertEquals(reply.errno, 0);
  assertEquals(new Uint32Array(reply.buf, reply.args_offset(), 1)[0], 42);
});

Deno.test("serve - without ACKREQ nothing comes back", async () => {
  const { client, replies } = served();
  client.send({ frame: request(make_tx_id(1, 2), ECHO, 0, 1), spill: null });
  await settle();
  assertEquals(replies.length, 0);
});

Deno.test("port - an unknown command, an XputeError and any other throw each reply with an errno and an empty body", async () => {
  const { client, replies } = served();
  client.send({ frame: request(make_tx_id(1, 3), 99, FrameFlag.ACKREQ), spill: null });
  client.send({ frame: request(make_tx_id(1, 4), FAIL, FrameFlag.ACKREQ), spill: null });
  const logged = console.error;
  console.error = () => {};
  try {
    client.send({ frame: request(make_tx_id(1, 5), THROW, FrameFlag.ACKREQ), spill: null });
    await settle();
  } finally {
    console.error = logged;
  }
  assertEquals(replies.map((m) => new Frame(m.frame).errno), [Errno.ENOENT, Errno.EBUSY, Errno.EIO]);
  for (const m of replies) {
    const f = new Frame(m.frame);
    assertEquals(f.args_bytes, 0);
    assertEquals(f.body_bytes, 0);
    assert(m.spill === null);
  }
});

Deno.test("frame - a body that does not fit is declared in a spill", () => {
  const frame = new Frame(new ArrayBuffer(64));
  frame.begin(1, 1, 0);
  assertEquals(frame.lay_out(4, 24), 40);
  assertEquals(frame.lay_out(4, 25), 0xffffffff);
  assertEquals(frame.body_bytes, 25);
});
