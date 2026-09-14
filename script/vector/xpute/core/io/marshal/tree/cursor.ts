// script/vector/xpute/core/io/marshal/tree/cursor.ts
//
// Packets the TypeScript encoder wrote and what its cursor reads from them
// — deep, and shallow at every child — the malformed packets it refuses,
// and the encoder's answer at a cap tight enough to refuse a string, for
// crates/xpute/core/src/io/marshal/tree/cursor.test.rs. Also a string and a
// bitset whose length word is 2^32 - 1, and ALIGN at the top of u32.

import { TreeEncoder, TreeReader, TreeView } from "@xpute/core/io/marshal/tree/mod.ts";
import { ALIGN } from "@xpute/core/io/marshal/tree/spec.ts";
import { bytes_to_hex } from "@xpute/core/kit/encoding.ts";

/** A value as one line: what both sides print a read value as. */
function show(v: unknown): string {
  if (v === null) return "null";
  if (Array.isArray(v)) return `[${v.map(show).join(",")}]`;
  if (ArrayBuffer.isView(v)) return `${v.constructor.name}(${Array.from(v as unknown as ArrayLike<number | bigint>).join(" ")})`;
  if (typeof v === "string") return `string:${bytes_to_hex(new TextEncoder().encode(v))}`;
  return `${typeof v}:${v}`;
}

function errno_of(f: () => unknown): string {
  try {
    f();
    return "ok";
  } catch (e) {
    return `errno:${(e as { errno?: number }).errno ?? "none"}`;
  }
}

export function vectors() {
  const trees: [string, (t: TreeView) => void][] = [
    ["mixed", (t) => t.set([42, 'hé"llo', true, null, 2n ** 40n, [1, [2.5, -3]], new Int32Array([7, -8]), new Float64Array([0.5])])],
    [
      "typed",
      (t) => t.branch((b) => b.u8(255).i8(-128).u16(65535).i16(-32768).u32(4294967295).i64(-1n).u64(18446744073709551615n).f32(1.1).bitset(new Uint8Array([1, 0, 1, 1, 0, 0, 0, 0, 1])).str("")),
    ],
    ["nulls", (t) => t.branch((b) => b.u32(null).str(null).branch(null).nil())],
    ["root_null", (t) => t.f64(null)],
    ["graft", (t) => t.branch((b) => b.graft(new TreeEncoder().encode(new TreeView().set(["inner", 1])).slice()).put(3))],
  ];
  const packets = trees.map(([name, build]) => {
    const t = new TreeView();
    build(t);
    const pkt = new TreeEncoder().encode(t);
    const root = new TreeReader(pkt).read();
    return {
      name,
      packet: bytes_to_hex(pkt),
      type: root.type,
      deep: root.is_branch() ? show(root.as_branch().get_deep()) : show(root.get()),
      shallow: root.is_branch() ? root.as_branch().children().map((c) => `${c.type}:${c.is_branch() ? "branch" : show(c.get())}`) : [],
    };
  });
  const bytes = (hex: string) => new Uint8Array(hex.match(/../g)!.map((h) => parseInt(h, 16)));
  const read_all = (hex: string) => () => {
    const r = new TreeReader(bytes(hex)).read();
    return r.is_branch() ? r.as_branch().get_deep() : r.get();
  };
  const good = packets[0].packet;
  const malformed = {
    truncated_header: errno_of(read_all(good.slice(0, 20))),
    bad_magic: errno_of(read_all("00" + good.slice(2))),
    truncated_payload: errno_of(read_all(good.slice(0, 40))),
  };
  const at_cap = (s: string, cap: number) => {
    try {
      return bytes_to_hex(new TreeEncoder().encode(new TreeView().str(s), { max_cap: cap }));
    } catch (e) {
      return `errno:${(e as { errno?: number }).errno}`;
    }
  };
  // The length word sits at the root's base, right after the 16-byte header.
  const huge_len = (t: TreeView) => {
    const hex = bytes_to_hex(new TreeEncoder().encode(t));
    return hex.slice(0, 32) + "ffffffff" + hex.slice(40);
  };
  const huge = [huge_len(new TreeView().str("ab")), huge_len(new TreeView().bitset(new Uint8Array([1, 0, 1])))].map((packet) => ({ packet, read: errno_of(read_all(packet)) }));
  const align = [0x7ffffff9, 0xfffffff8, 0xfffffff9].map((n) => {
    try {
      return `ok:${ALIGN(n, 8)}`;
    } catch (e) {
      return `errno:${(e as { errno?: number }).errno}`;
    }
  });
  return {
    packets,
    malformed,
    huge,
    align,
    caps: [24, 32, 40, 48].map((cap) => at_cap("abcdefgh", cap)),
    caps_utf8: [24, 32, 40].map((cap) => at_cap("한글한", cap)),
  };
}
