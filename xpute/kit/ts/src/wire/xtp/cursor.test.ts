// @xpute/kit/wire/xtp/cursor.test.ts

/** The TypeScript side of xpute-kit/wire/xtp/cursor.test.rs, held to the
 * same golden record, spec/golden/wire/xtp/cursor.tsv. */

import { test } from "vitest";
import assert from "node:assert/strict";

import { Golden } from "@xpute/golden";
import { bytes_to_hex, hex_to_bytes } from "@xpute/kit/codec/encoding.ts";
import type { XputeError } from "@xpute/kit/status/error.ts";
import { type NodeCursor, TreeReader } from "@xpute/kit/wire/xtp/cursor.ts";
import { TreeEncoder } from "@xpute/kit/wire/xtp/encode.ts";
import { ALIGN, type NodeValue } from "@xpute/kit/wire/xtp/spec.ts";
import { TreeView } from "@xpute/kit/wire/xtp/view.ts";

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
    return `errno:${(err as XputeError).errnum}`;
  }
}

test("the cursor reads what the typescript reads", async () => {
  const v = await Golden.load("kit/spec/golden/wire/xtp/cursor.tsv");

  v.each("pkts", (k) => {
    const root = new TreeReader(hex_to_bytes(v.s(`${k}.pkt`))).root;
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

  const good = v.s("pkts.0.pkt");
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
    assert.deepStrictEqual(refused(v.s(`${k}.pkt`)), v.s(`${k}.read`), k),
  );

  [0x7ffffff9, 0xfffffff8, 0xfffffff9].forEach((n, j) => {
    let got: string;
    try {
      got = `ok:${ALIGN(n, 8)}`;
    } catch (err) {
      got = `errno:${(err as XputeError).errnum}`;
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
      return `errno:${(err as XputeError).errnum}`;
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
