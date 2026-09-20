// @xpute/runtime/ipc/ring.test.ts

import { assert, assertEquals, assertThrows } from "@std/assert";

import { Errno } from "@xpute/core/status/errno.spec.ts";
import { encoder, TreeReader, TreeView } from "@xpute/core/wire/xtp/mod.ts";
import { Ring, ring_bytes, slots_bytes } from "./ring.ts";
import { FrameFlag } from "./frame.ts";

/** A ring of `capacity` slots of `slot` bytes, its descriptors at 8 and its
 * payloads past them, in a buffer of its own. */
function ring(capacity: number, slot: number): Ring {
  const slot_base = 8 + ring_bytes(capacity);
  return Ring.init(new ArrayBuffer(slot_base + slots_bytes(capacity, slot)), 8, capacity, slot_base, slot);
}

Deno.test("ring - messages come out in order across the wrap, and a full ring refuses", () => {
  const r = ring(4, 64);
  for (let round = 0; round < 3; round++) {
    for (let k = 0; k < 4; k++) assertEquals(r.push(round * 4 + k, 7, FrameFlag.ACKREQ, null, -(round * 4 + k)), Errno.OK);
    assertEquals(r.push(99, 0), Errno.EAGAIN, "a full ring refuses");
    for (let k = 0; k < 4; k++) {
      const at = r.peek();
      assert(at >= 0);
      assertEquals([r.tag(at), r.cmd(at), r.flags(at), r.result(at), r.packet(at)], [round * 4 + k, 7, FrameFlag.ACKREQ, -(round * 4 + k), null]);
      r.advance();
    }
    assertEquals(r.peek(), -1);
  }
  assertEquals([r.head, r.tail], [12, 12], "positions only grow");
});

Deno.test("ring - one payload a slot, so the payloads run out with the slots and never before them", () => {
  const r = ring(8, 64);
  const pkt = (n: number) => encoder.encode(new TreeView().branch((b) => b.u32(n).u64(BigInt(n) << 40n)));
  assertEquals(pkt(1).byteLength, 56);

  for (let n = 1; n <= 8; n++) assertEquals(r.push(n, 0x0101, 0, pkt(n)), Errno.OK, "every slot has a payload of its own");
  assertEquals(r.push(9, 0x0101, 0, pkt(9)), Errno.EAGAIN, "and the ring fills before the payloads do");
  assertEquals(r.len, 8);

  for (let n = 1; n <= 8; n++) {
    const at = r.peek();
    const root = new TreeReader(r.packet(at)!).read_branch();
    assertEquals([r.tag(at), root.at(0).get<number>(), root.at(1).get<bigint>()], [n, n, BigInt(n) << 40n]);
    r.advance();
  }
  assertEquals(r.push(9, 0x0101, 0, pkt(9)), Errno.OK, "a slot freed is a payload freed");

  assertEquals(r.push(10, 0x0101, 0, new Uint8Array(72)), Errno.EMSGSIZE);
  assertThrows(
    () => {
      const at = r.entry_at(r.tail);
      r.push(11, 0);
      r.words[at + 3] = 4;
      r.packet(at);
    },
    Error,
    "outside a slot",
  );
});
