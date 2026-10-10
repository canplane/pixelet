// @xpute/core/ipc/ring.test.ts

import { assert, assertEquals, assertThrows } from "@std/assert";

import { Errno } from "@xpute/core/sys/errno.ts";
import { encoder, TreeReader, TreeView } from "@xpute/core/io/marshal/tree/mod.ts";
import { Ring, ring_bytes } from "./ring.ts";
import { FrameFlag } from "./frame.ts";

Deno.test("ring - messages come out in order across the wrap, and a full ring refuses", () => {
  const buf = new ArrayBuffer(ring_bytes(4, 64) + 8);
  const r = Ring.init(buf, 8, 4, 64);
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

Deno.test("ring - a packet is read where it lies, the arena rewinds once the ring is empty, and one too large is refused", () => {
  const buf = new ArrayBuffer(ring_bytes(8, 128));
  const r = Ring.init(buf, 0, 8, 128);
  const pkt = (n: number) => encoder.encode(new TreeView().branch((b) => b.u32(n).u64(BigInt(n) << 40n)));
  assertEquals(pkt(1).byteLength, 56);

  assertEquals(r.push(1, 0x0101, 0, pkt(1)), Errno.OK);
  assertEquals(r.push(2, 0x0101, 0, pkt(2)), Errno.OK);
  assertEquals(r.push(3, 0x0101, 0, pkt(3)), Errno.EAGAIN, "the arena has no room for a third");
  assertEquals(r.len, 2, "and nothing was written");

  for (const n of [1, 2]) {
    const at = r.peek();
    const root = new TreeReader(r.packet(at)!).read_branch();
    assertEquals([r.tag(at), root.at(0).get<number>(), root.at(1).get<bigint>()], [n, n, BigInt(n) << 40n]);
    r.advance();
  }
  assertEquals(r.push(3, 0x0101, 0, pkt(3)), Errno.OK, "an empty ring's arena starts over");
  assertEquals(new TreeReader(r.packet(r.peek())!).read_branch().at(0).get<number>(), 3);
  r.advance();

  assertEquals(r.push(4, 0x0101, 0, new Uint8Array(136)), Errno.EMSGSIZE);
  assertThrows(
    () => {
      const at = r.entry_at(r.tail);
      r.push(5, 0);
      r.words[at + 3] = 4;
      r.packet(at);
    },
    Error,
    "outside the arena",
  );
});
