// @xpute/core/wire/xtp/cursor.test.ts

/**
 * The half of xpute-core/wire/xtp/cursor.test.rs that runs here. A packet is
 * written by one side and read by the other, so the record is a claim about
 * both and each has to be held to it.
 *
 * No aligned buffer is needed where the Rust builds one: a fresh `Uint8Array`
 * begins at offset 0 of its own buffer, which is what the reader asks for.
 */

import { assertEquals } from "@std/assert";

import { Golden } from "@xpute/core/golden.ts";
import { bytes_to_hex, hex_to_bytes } from "@xpute/core/codec/encoding.ts";
import type { FaultError } from "@xpute/core/status/error.ts";
import { ALIGN, type NodeCursor, type NodeValue, TreeEncoder, TreeReader, TreeView } from "@xpute/core/wire/xtp/mod.ts";

const te = new TextEncoder();

/** A value as the record prints it. */
function show(v: NodeValue): string {
  if (v === null) return "null";
  if (Array.isArray(v)) return `[${v.map(show).join(",")}]`;
  if (ArrayBuffer.isView(v)) {
    const name = v.constructor.name;
    return `${name}(${Array.from(v as unknown as ArrayLike<number | bigint>).join(" ")})`;
  }
  if (typeof v === "string") return `string:${bytes_to_hex(te.encode(v))}`;
  return `${typeof v}:${v}`;
}

/** A cursor read right through, whichever kind it is. */
function deep(c: NodeCursor): NodeValue {
  return c.is_branch() ? c.get_deep() : c.get();
}

/** What reading `hex` right through comes to: the value, or the errno refusing it. */
function refused(hex: string): string {
  try {
    deep(new TreeReader(hex_to_bytes(hex)).root);
    return "ok";
  } catch (err) {
    return `errno:${(err as FaultError).errno}`;
  }
}

Deno.test("the cursor reads what the typescript reads", async () => {
  const v = await Golden.load("wire/xtp/cursor.tsv");

  v.each("packets", (k) => {
    const root = new TreeReader(hex_to_bytes(v.s(`${k}.packet`))).root;
    assertEquals(root.type, v.u32(`${k}.type`), `${k}.type`);
    assertEquals(show(deep(root)), v.s(`${k}.deep`), `${k}.deep`);
    if (root.is_branch()) {
      const got = root.children().map((c) => `${c.type}:${c.is_branch() ? "branch" : show(c.get())}`);
      const want: string[] = [];
      if (v.has(`${k}.shallow.0`)) v.each(`${k}.shallow`, (j) => want.push(v.s(j)));
      assertEquals(got, want, `${k}.shallow`);
    }
  });

  const good = v.s("packets.0.packet");
  assertEquals(refused(good.slice(0, 20)), v.s("malformed.truncated_header"));
  assertEquals(refused(`00${good.slice(2)}`), v.s("malformed.bad_magic"));
  assertEquals(refused(good.slice(0, 40)), v.s("malformed.truncated_payload"));
  v.each("huge", (k) => assertEquals(refused(v.s(`${k}.packet`)), v.s(`${k}.read`), k));

  [0x7ffffff9, 0xfffffff8, 0xfffffff9].forEach((n, j) => {
    let got: string;
    try {
      got = `ok:${ALIGN(n, 8)}`;
    } catch (err) {
      got = `errno:${(err as FaultError).errno}`;
    }
    assertEquals(got, v.s(`align.${j}`), `ALIGN(${n})`);
  });

  const at_cap = (s: string, cap: number): string => {
    const t = new TreeView();
    t.str(s);
    try {
      return bytes_to_hex(new TreeEncoder().encode(t, { max_cap: cap }));
    } catch (err) {
      return `errno:${(err as FaultError).errno}`;
    }
  };
  [24, 32, 40, 48].forEach((cap, j) => assertEquals(at_cap("abcdefgh", cap), v.s(`caps.${j}`), `caps.${j} at ${cap}`));
  [24, 32, 40].forEach((cap, j) => assertEquals(at_cap("한글한", cap), v.s(`caps_utf8.${j}`), `caps_utf8.${j} at ${cap}`));
});
