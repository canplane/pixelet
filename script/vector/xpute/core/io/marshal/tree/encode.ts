// script/vector/xpute/core/io/marshal/tree/encode.ts
//
// What the TypeScript encoder writes for a fixed set of trees, as hex: the
// Rust encoder must write the same bytes (crates/xpute-core/src/io/marshal/tree/encode.rs).

import { TreeEncoder, TreeView } from "@xpute/core/io/marshal/tree/mod.ts";
import { bytes_to_hex } from "@xpute/core/kit/encoding.ts";

export function vectors(): { name: string; packet: string }[] {
  const enc = new TreeEncoder();
  const cases: [string, (t: TreeView) => void][] = [
    ["nil", (t) => t.nil()],
    ["i32", (t) => t.set(42)],
    ["f64", (t) => t.set(1.5)],
    ["bigint", (t) => t.set(-5n)],
    ["u64", (t) => t.u64(0xffff_ffff_ffff_fffen)],
    ["bool", (t) => t.set(true)],
    ["str", (t) => t.set("héllo")],
    ["str empty", (t) => t.str("")],
    ["typed u16", (t) => t.set(new Uint16Array([1, 2, 3]))],
    ["typed f32", (t) => t.f32_array(new Float32Array([0.5, -2]))],
    ["bitset", (t) => t.bitset(new Uint8Array([1, 0, 1, 1, 0, 0, 0, 0, 1]))],
    ["branch mixed", (t) => t.set([42, "hello", true, null, 2n ** 40n, [1, 2], new Int32Array([7])])],
    ["branch null child", (t) => t.branch((b) => b.u8(null).str(null).branch(null).u32(9))],
    ["branch empty", (t) => t.set([])],
    ["typed null root", (t) => t.u32(null)],
    ["graft", (t) => t.graft(enc.encode(new TreeView().set([1, "x"])).slice())],
  ];
  return cases.map(([name, build]) => {
    const t = new TreeView();
    build(t);
    return { name, packet: bytes_to_hex(new TreeEncoder().encode(t)) };
  });
}
