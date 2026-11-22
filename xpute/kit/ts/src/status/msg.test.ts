// @xpute/kit/status/msg.test.ts

import { test } from "vitest";
import assert from "node:assert/strict";

import { Golden } from "@xpute/golden";
import { bytes_to_hex, te } from "@xpute/kit/codec/encoding.ts";
import { cut_msg, MSG_CAP } from "./msg.ts";

test("a message within the cap is whole, and one past it is cut before the character the cap falls in", () => {
  assert.equal(cut_msg(""), "");
  const whole = "é".repeat(MSG_CAP / 2);
  assert.equal(cut_msg(whole), whole);
  assert.equal(cut_msg(`x${whole}`), `x${whole}`.slice(0, MSG_CAP / 2));
});

test("each recorded message is cut as the record holds it, a lone surrogate as the bytes it is encoded to", async () => {
  const g = await Golden.load("kit/spec/golden/status/msg.tsv");
  const lead = "x".repeat(MSG_CAP - 2);
  for (const [k, msg] of [
    ["straddle", `${lead}😀tail`],
    ["surrogate", `${lead}\ud800tail`],
  ] as const) {
    assert.equal(
      bytes_to_hex(te.encode(msg)),
      bytes_to_hex(g.bytes(`${k}.in`)),
    );
    assert.equal(
      bytes_to_hex(te.encode(cut_msg(msg))),
      bytes_to_hex(g.bytes(`${k}.out`)),
      k,
    );
  }
});
