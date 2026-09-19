// @xpute/core/wire/xtp/encode.test.ts

/**
 * The half of xpute-core/wire/xtp/encode.test.rs that runs here: the same
 * shapes encoded, held to the same bytes. What one side writes the other
 * reads, so a packet's bytes are the contract and the record is what says the
 * two still agree on them.
 */

import { assertEquals } from "@std/assert";

import { Golden } from "@xpute/core/golden.ts";
import type { U8Array } from "@xpute/core/abi/array.ts";
import { bytes_to_hex } from "@xpute/core/codec/encoding.ts";
import { TreeEncoder, TreeView } from "@xpute/core/wire/xtp/mod.ts";

/** The vector the record keeps under `name`. */
function expected(v: Golden, name: string): string {
  for (let k = 0;; k++) {
    if (!v.has(`${k}.name`)) throw new Error(`no vector ${name}`);
    if (v.s(`${k}.name`) === name) return v.s(`${k}.packet`);
  }
}

function encode(build: (t: TreeView) => void): U8Array {
  const t = new TreeView();
  build(t);
  return new TreeEncoder().encode(t);
}

Deno.test("the encoder writes the bytes the typescript writes", async () => {
  const v = await Golden.load("wire/xtp/encode.tsv");
  const check = (name: string, build: (t: TreeView) => void) => assertEquals(bytes_to_hex(encode(build)), expected(v, name), name);

  check("nil", (t) => t.nil());
  check("i32", (t) => t.set(42));
  check("f64", (t) => t.set(1.5));
  check("bigint", (t) => t.set(-5n));
  check("u64", (t) => t.u64(0xffff_ffff_ffff_fffen));
  check("bool", (t) => t.set(true));
  check("str", (t) => t.set("héllo"));
  check("str empty", (t) => t.str(""));
  check("typed u16", (t) => t.set(new Uint16Array([1, 2, 3])));
  check("typed f32", (t) => t.f32_array(new Float32Array([0.5, -2.0])));
  check("bitset", (t) => t.bitset(new Uint8Array([1, 0, 1, 1, 0, 0, 0, 0, 1]) as U8Array));
  check("branch mixed", (t) => t.set([42, "hello", true, null, 1n << 40n, [1, 2], new Int32Array([7])]));
  check("branch null child", (t) =>
    t.branch((b) => {
      b.u8(null).str(null).branch(null).u32(9);
    }));
  check("branch empty", (t) => t.set([]));
  check("typed null root", (t) => t.u32(null));

  const inner = encode((t) => t.set([1, "x"]));
  check("graft", (t) => t.graft(inner));
});
