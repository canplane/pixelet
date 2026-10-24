// @xpute/hypervisor/ipc/ring.test.ts

import { assert, assertEquals, assertThrows } from "@std/assert";

import { Errno } from "@xpute/kit/status/errno.spec.ts";
import { PacketWriter, TreeReader } from "@xpute/kit/wire/xtp/mod.ts";
import { Ring, ring_bytes, slots_bytes } from "./ring.ts";
import * as frame from "./frame.ts";
import { MarshalError } from "@xpute/kit/status/error.ts";

function ring(capacity: number, slot: number): Ring {
  const slot_base = 8 + ring_bytes(capacity);
  return Ring.init(new ArrayBuffer(slot_base + slots_bytes(capacity, slot)), 8, capacity, slot_base, slot);
}

Deno.test("ring - messages come out in order across the wrap, and a full ring refuses", () => {
  const r = ring(4, 64);
  for (let round = 0; round < 3; round++) {
    for (let k = 0; k < 4; k++) assertEquals(r.push(round * 4 + k, 7, frame.ACKREQ, null, -(round * 4 + k)), Errno.OK);
    assertEquals(r.push(99, 0), Errno.EAGAIN, "a full ring refuses");
    for (let k = 0; k < 4; k++) {
      const at = r.peek();
      assert(at >= 0);
      assertEquals([r.tag(at), r.cmd(at), r.flags(at), r.result(at), r.packet(at)], [round * 4 + k, 7, frame.ACKREQ, -(round * 4 + k), null]);
      r.advance();
    }
    assertEquals(r.peek(), -1);
  }
  assertEquals([r.head, r.tail], [12, 12], "positions only grow");
});

Deno.test("ring - one payload a slot, so the payloads run out with the slots and never before them", () => {
  const r = ring(8, 64);
  const w = new PacketWriter();
  const push = (n: number) => r.write(w, 2) ? r.push(n, 0x0101, 0, w.u32(n).f64(n * 2 ** 40)) : Errno.EAGAIN;

  for (let n = 1; n <= 8; n++) assertEquals(push(n), Errno.OK, "every slot has a payload of its own");
  assertEquals(push(9), Errno.EAGAIN, "and the ring fills before the payloads do");
  assertEquals(r.len, 8);

  for (let n = 1; n <= 8; n++) {
    const at = r.peek();
    assertEquals(r.packet(at)!.byteLength, 56);
    const root = new TreeReader(r.packet(at)!).read_branch();
    assertEquals([r.tag(at), root.at(0).get<number>(), root.at(1).get<number>()], [n, n, n * 2 ** 40]);
    r.advance();
  }
  assertEquals(push(9), Errno.OK, "a slot freed is a payload freed");

  assert(r.write(w, 1));
  assertEquals(r.push(10, 0x0101, 0, w.str("x".repeat(40))), Errno.EMSGSIZE);
  assertThrows(
    () => {
      const at = r.entry_at(r.tail);
      r.push(11, 0);
      r.words[at + 3] = 4;
      r.packet(at);
    },
    MarshalError,
  );
});
