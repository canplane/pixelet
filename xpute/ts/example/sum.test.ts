// @xpute/example/sum.test.ts

/**
 * The smallest host, beside the smallest guest (rust/example): the guest's
 * module opened and mounted in one call (host/port.ts `open`), with an answer
 * to the one hostcall the guest makes, then one command called with its table's
 * writer and its event heard with its table's reader (cmd.spec.ts). Build the
 * guest first (`make build`).
 */

import { test } from "vitest";
import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import { join, resolve } from "node:path";
import { fileURLToPath } from "node:url";

import { open } from "@xpute/host/port.ts";
import { SUM, SUMMED } from "./cmd.spec.ts";

const ROOT = fileURLToPath(new URL("../../", import.meta.url));
/** A relative CARGO_TARGET_DIR is cargo's, which the root's tasks run from rust/. */
const CARGO_TARGET = resolve(
  join(ROOT, "rust"),
  process.env.CARGO_TARGET_DIR ?? "target",
);
const GUEST = join(
  CARGO_TARGET,
  "wasm32-unknown-unknown/debug/xpute_example.wasm",
);

/** The hostcall the guest makes (rust/example/src/lib.rs `ASK`). */
const ASK = 1;

/** What the host holds and the guest asks for. */
const HELD = 100;

test("example - a command answered with what the guest asked its host", () => {
  const guest = open(new WebAssembly.Module(readFileSync(GUEST)), {
    answer: { [ASK]: () => HELD },
  });
  let summed = -1;
  guest.on(SUMMED, (total) => {
    summed = total;
  });
  assert.deepStrictEqual(guest.call(SUM, 2, 3), 2 + 3 + HELD);
  assert.deepStrictEqual(summed, 2 + 3);
});
