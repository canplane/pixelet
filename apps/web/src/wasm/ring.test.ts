// @pxl/web/wasm/ring.test.ts

import { assert, assertEquals } from "@std/assert";

import { Errno } from "@xpute/core/sys/errno.ts";
import { encoder, TreeView } from "@xpute/core/io/marshal/tree/mod.ts";
import { FrameFlag } from "@xpute/core/ipc/frame.ts";
import { Ring } from "@xpute/core/ipc/ring.ts";
import { Interrupt } from "@xpute/core/ipc/cmd.ts";
import { Cmd } from "@pxl/shared/spec/cmd.ts";

import { CORE_KERNEL } from "@/wasm/core_memory.ts";
import { BOOT, boot_block } from "@/wasm/module.ts";

/** A command crates/pxl/wasm/src/lib.rs does not number. */
const UNNUMBERED = 0x0fff;

Deno.test("rings - the glue submits, one drain replies to each in order, and a full completion ring holds the rest back", () => {
  assert(CORE_KERNEL !== null);
  const p = CORE_KERNEL;
  const boot = boot_block(p);
  const sq = new Ring(p.memory.buffer, boot[BOOT.sq]);
  const cq = new Ring(p.memory.buffer, boot[BOOT.cq]);
  assertEquals([sq.capacity, cq.capacity], [4096, 4096]);

  const n = sq.capacity;
  for (let k = 0; k < n; k++) {
    assertEquals(sq.push(k, Cmd.NOP, FrameFlag.ACKREQ, encoder.encode(new TreeView().branch((b) => b.u32(k * 3)))), Errno.OK);
  }
  assertEquals(sq.push(n, Cmd.NOP, FrameFlag.ACKREQ), Errno.EAGAIN, "a full submission ring refuses");
  assertEquals(p.interrupt(Interrupt.DRAIN), n);
  assertEquals(sq.len, 0);

  // The completion ring is now full: new submissions are applied, and what
  // they make waits in the module for room.
  for (let k = 0; k < 5; k++) sq.push(1000 + k, UNNUMBERED);
  assertEquals(p.interrupt(Interrupt.DRAIN), 0, "nothing completes into a full ring");
  assertEquals(sq.len, 0, "they are applied; their replies wait in the module, and nothing is dropped");

  for (let k = 0; k < n; k++) {
    const at = cq.peek();
    assertEquals([cq.tag(at), cq.cmd(at), cq.flags(at), cq.result(at)], [k, Cmd.NOP, FrameFlag.RES, k * 3]);
    cq.advance();
  }
  assertEquals(p.interrupt(Interrupt.DRAIN), 5, "a command nobody numbers replies without being asked to");
  const at = cq.peek();
  assertEquals([cq.tag(at), cq.cmd(at), cq.flags(at), cq.result(at)], [1000, UNNUMBERED, FrameFlag.RES, Errno.ENOSYS]);
  while (cq.peek() >= 0) cq.advance();
  assertEquals(p.interrupt(0xff) >>> 0, 0xffff_ffff, "a vector nobody numbers");
});
