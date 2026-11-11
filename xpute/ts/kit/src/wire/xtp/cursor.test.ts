// @xpute/kit/wire/xtp/cursor.test.ts

/** The TypeScript side of xpute-kit/wire/xtp/cursor.test.rs, held to the
 * same golden record, spec/golden/wire/xtp/cursor.tsv. */

import { test } from "vitest";
import assert from "node:assert/strict";

import { Golden } from "../../../../dev/golden.ts";
import { bytes_to_hex, hex_to_bytes } from "@xpute/kit/codec/encoding.ts";
import * as Errno from "@xpute/kit/status/errno.spec.ts";
import type { FaultError, MarshalError } from "@xpute/kit/status/error.ts";
import {
  ALIGN,
  PacketWriter,
  SequenceType,
  type NodeCursor,
  type NodeValue,
  TreeEncoder,
  TreeReader,
  TreeView,
} from "@xpute/kit/wire/xtp/mod.ts";

const te = new TextEncoder();

function show(v: NodeValue): string {
  if (v === null) return "null";
  if (Array.isArray(v)) return `[${v.map(show).join(",")}]`;
  if (ArrayBuffer.isView(v)) {
    const name = v.constructor.name;
    return `${name}(${Array.from(
      v as unknown as ArrayLike<number | bigint>,
    ).join(" ")})`;
  }
  if (typeof v === "string") return `string:${bytes_to_hex(te.encode(v))}`;
  return `${typeof v}:${v}`;
}

function deep(c: NodeCursor): NodeValue {
  return c.is_branch() ? c.get_deep() : c.get();
}

function refused(hex: string): string {
  try {
    deep(new TreeReader(hex_to_bytes(hex)).root);
    return "ok";
  } catch (err) {
    return `errno:${(err as FaultError).errnum}`;
  }
}

test("the cursor reads what the typescript reads", async () => {
  const v = await Golden.load("golden/wire/xtp/cursor.tsv");

  v.each("packets", (k) => {
    const root = new TreeReader(hex_to_bytes(v.s(`${k}.packet`))).root;
    assert.deepStrictEqual(root.type, v.u32(`${k}.type`), `${k}.type`);
    assert.deepStrictEqual(show(deep(root)), v.s(`${k}.deep`), `${k}.deep`);
    if (root.is_branch()) {
      const got = root
        .children()
        .map((c) => `${c.type}:${c.is_branch() ? "branch" : show(c.get())}`);
      const want: string[] = [];
      if (v.has(`${k}.shallow.0`)) {
        v.each(`${k}.shallow`, (j) => want.push(v.s(j)));
      }
      assert.deepStrictEqual(got, want, `${k}.shallow`);
    }
  });

  const good = v.s("packets.0.packet");
  assert.deepStrictEqual(
    refused(good.slice(0, 20)),
    v.s("malformed.truncated_hdr"),
  );
  assert.deepStrictEqual(
    refused(`00${good.slice(2)}`),
    v.s("malformed.bad_magic"),
  );
  assert.deepStrictEqual(
    refused(good.slice(0, 40)),
    v.s("malformed.truncated_payload"),
  );
  v.each("refused", (k) =>
    assert.deepStrictEqual(refused(v.s(`${k}.packet`)), v.s(`${k}.read`), k),
  );

  [0x7ffffff9, 0xfffffff8, 0xfffffff9].forEach((n, j) => {
    let got: string;
    try {
      got = `ok:${ALIGN(n, 8)}`;
    } catch (err) {
      got = `errno:${(err as FaultError).errnum}`;
    }
    assert.deepStrictEqual(got, v.s(`align.${j}`), `ALIGN(${n})`);
  });

  const at_cap = (s: string, cap: number): string => {
    const t = new TreeView();
    t.str(s);
    try {
      return bytes_to_hex(
        new TreeEncoder().encode(t, { init_cap: 1 << 10, max_cap: cap }),
      );
    } catch (err) {
      return `errno:${(err as FaultError).errnum}`;
    }
  };
  [24, 32, 40, 48].forEach((cap, j) =>
    assert.deepStrictEqual(
      at_cap("abcdefgh", cap),
      v.s(`caps.${j}`),
      `caps.${j} at ${cap}`,
    ),
  );
  [24, 32, 40].forEach((cap, j) =>
    assert.deepStrictEqual(
      at_cap("€€€", cap),
      v.s(`caps_utf8.${j}`),
      `caps_utf8.${j} at ${cap}`,
    ),
  );
});

/** A packet of 7, "a", true, a packed u32 array and an absent field. */
function kinds() {
  const buf = new Uint8Array(256);
  const w = new PacketWriter().begin(buf, new DataView(buf.buffer), 0, 256, 5);
  w.u32(7)
    .str("a")
    .bool(true)
    .array(SequenceType.U32_ARRAY, new Uint32Array([1, 2]))
    .u32(null);
  return new TreeReader(buf.subarray(0, w.finish())).read_branch();
}

test("a checked getter reads its kind, and refuses another kind with EBADMSG, as xpute-kit's cursor does", () => {
  const b = kinds();
  assert.deepStrictEqual(
    [b.at(0).get_number(), b.at(1).get_str(), b.at(2).get_bool()],
    [7, "a", true],
  );
  assert.deepStrictEqual([...b.at(3).get_u32s()], [1, 2]);
  assert.deepStrictEqual(
    [
      b.at(0).get_opt((c) => c.get_number()),
      b.at(4).get_opt((c) => c.get_number()),
    ],
    [7, null],
  );
  const refused = (e: MarshalError) => e.errnum === Errno.EBADMSG;
  for (const get of [
    () => b.at(1).get_number(),
    () => b.at(0).get_str(),
    () => b.at(0).get_bool(),
    () => b.at(0).get_u32s(),
    () => b.at(4).get_number(),
  ]) {
    assert.throws(get, refused);
  }
});
