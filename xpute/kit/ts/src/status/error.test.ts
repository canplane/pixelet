// @xpute/kit/status/error.test.ts

import { test } from "vitest";
import assert from "node:assert/strict";

import { Golden } from "@xpute/golden";
import { td } from "@xpute/kit/codec/encoding.ts";
import { bug, XputeBug } from "./bug.ts";
import { ERRNO_EIO } from "./errno.spec.ts";
import { frames, XputeError } from "./error.ts";
import { MSG_CAP } from "./msg.ts";

test("an error is placed where it is made; its message past the cap is cut at a character boundary, and an empty one is none", () => {
  // The place is the frame that constructs it, read by its position and by
  // no file's name, so it holds where every frame is one file's, as a
  // bundle's are: here the maker and its caller are both this file's.
  // prettier-ignore
  const [here, e] = [new Error(), new XputeError(ERRNO_EIO, "no café")];
  assert.deepEqual([e.file, e.line], frames(here.stack ?? "")[0]);
  const make = () => new XputeError(ERRNO_EIO);
  const [caller, made] = [new Error(), make()];
  assert.equal(made.file, e.file);
  assert.equal(made.line, frames(caller.stack ?? "")[0]![1] - 1);
  assert.deepEqual([e.msg, e.message], ["no café", "no café"]);
  const long = `x${"é".repeat(MSG_CAP / 2)}`;
  const cut = long.slice(0, MSG_CAP / 2);
  assert.equal(new XputeError(ERRNO_EIO, long).msg, cut);
  assert.equal(new XputeError(ERRNO_EIO, "").msg, null);
  const came = new XputeError(ERRNO_EIO, null, ["a/b.rs", 7]);
  assert.deepEqual([came.file, came.line, came.msg], ["a/b.rs", 7, null]);
  assert.equal(new XputeError(ERRNO_EIO, null, [long, 7]).file, cut);
});

test("a bug's message past the cap is cut as the record of the cut holds it, and an empty one is none", async () => {
  const g = await Golden.load("kit/spec/golden/status/msg.tsv");
  const [long, cut] = ["straddle.in", "straddle.out"].map((k) =>
    td.decode(g.bytes(k)),
  ) as [string, string];
  const caught = (msg: string): XputeBug => {
    try {
      bug(ERRNO_EIO, 0n, 0n, msg);
    } catch (e) {
      if (e instanceof XputeBug) return e;
      throw e;
    }
  };
  assert.equal(caught(long).msg, cut);
  assert.equal(caught("").msg, null);
  assert.ok(caught("full").file.endsWith("error.test.ts"));
});
