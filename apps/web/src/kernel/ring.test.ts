// @pxl/web/kernel/ring.test.ts

import { assert, assertEquals } from "@std/assert";

import { Errno } from "@xpute/core/status/errno.spec.ts";
import { encoder, TreeView } from "@xpute/core/wire/xtp/mod.ts";
import { FrameFlag } from "@xpute/runtime/ipc/frame.ts";
import { key } from "@xpute/runtime/ipc/directory.spec.ts";
import { Ring } from "@xpute/runtime/ipc/ring.ts";
import { Command } from "@/kernel/abi/cmd.spec.ts";

import { CORE, load_core } from "@/kernel/mem.ts";
import { directory } from "@/kernel/module.ts";
import { COMPLETION_CAPACITY, SUBMISSION_CAPACITY } from "@/kernel/mem/mem.spec.ts";

// The module the test reads, loaded before it runs.
await load_core();

/** A command crates/pxl/web/kernel/src/door/apply.rs does not number. */
const UNNUMBERED = 0x0fff;

/** `n` NOPs asking for a reply, tagged from `from`, each carrying its tag
 * three times over for the reply to echo. */
function push_nops(sq: Ring, from: number, n: number): void {
  for (let k = from; k < from + n; k++) {
    assertEquals(sq.push(k, Command.NOP, FrameFlag.ACKREQ, encoder.encode(new TreeView().branch((b) => b.u32(k * 3)))), Errno.OK);
  }
}

/** Reads the completion ring dry, checking each reply is the next NOP's. */
function read_nops(cq: Ring, from: number): number {
  let k = from;
  for (let at = cq.peek(); at >= 0; at = cq.peek()) {
    assertEquals([cq.tag(at), cq.cmd(at), cq.flags(at), cq.result(at)], [k, Command.NOP, FrameFlag.RES, k * 3]);
    cq.advance();
    k++;
  }
  return k;
}

Deno.test("rings - one turn answers every submission in order, and the kernel holds back what a host that did not read leaves no room to answer", () => {
  assert(CORE !== undefined);
  const p = CORE;
  const dir = directory(p.buffer);
  const sq = new Ring(p.buffer, dir.get(key.SUBMISSION));
  const cq = new Ring(p.buffer, dir.get(key.COMPLETION));
  assertEquals([sq.capacity, cq.capacity], [SUBMISSION_CAPACITY, COMPLETION_CAPACITY]);

  push_nops(sq, 0, sq.capacity);
  assertEquals(sq.push(sq.capacity, Command.NOP, FrameFlag.ACKREQ), Errno.EAGAIN, "a full submission ring refuses");
  p.interrupt(0);
  assertEquals([sq.len, cq.len], [0, sq.capacity], "one turn applies everything sent");

  // A host that rings again without reading breaks the contract's rule 5;
  // the kernel still overruns nothing. The completion ring has room for
  // fewer than another ring's worth, so the turn applies what it can answer
  // and leaves the rest, in order, asking for the next turn. What a turn
  // does when its own output fills the ring is xpute-runtime's
  // ipc/doorbell.test.rs.
  push_nops(sq, sq.capacity, sq.capacity);
  const wake = p.interrupt(0);
  assert(sq.len > 0, "what could not be answered is not applied");
  assert(cq.len < cq.capacity, "nothing is posted past the ring's room");
  assertEquals(wake, 0, "a turn that left submissions asks for the next");

  let next = read_nops(cq, 0);
  while (sq.len > 0) {
    p.interrupt(0);
    next = read_nops(cq, next);
  }
  assertEquals(next, 2 * sq.capacity, "every submission is answered once, in the order sent");

  for (let k = 0; k < 5; k++) sq.push(1000 + k, UNNUMBERED);
  p.interrupt(0);
  assertEquals(cq.len, 5, "a command nobody numbers replies without being asked to");
  const at = cq.peek();
  assertEquals([cq.tag(at), cq.cmd(at), cq.flags(at), cq.result(at)], [1000, UNNUMBERED, FrameFlag.RES, Errno.ENOSYS]);
  while (cq.peek() >= 0) cq.advance();
});
