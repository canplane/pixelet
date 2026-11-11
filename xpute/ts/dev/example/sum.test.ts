// @xpute/example/sum.test.ts

/**
 * The smallest runtime, beside the smallest core (rust/dev/example). It boots the
 * core as the core's manifest declares (manifest.spec.ts): the module is read
 * from disk and opened over a memory of the one size the manifest allows, with
 * a policy for a runtime that has no display. The core's one protocol
 * (asker.spec.ts) gives the program's handler, which answers the core's call.
 * The test then sends one command through the protocol's generated exports, as
 * the page does (web/main.ts), and reads its reply. Build the core first
 * (`make build`).
 */

import { test } from "vitest";
import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import { join, resolve } from "node:path";
import { fileURLToPath } from "node:url";

import { boot, QUIET, Timeslice } from "@xpute/runtime";
import { asker_exports, asker_imports, Command } from "./asker.spec.ts";
import { MANIFEST } from "./manifest.spec.ts";

const ROOT = fileURLToPath(new URL("../../../", import.meta.url));
/** A relative CARGO_TARGET_DIR is relative to rust/, where the root's tasks
 * run cargo. */
const CARGO_TARGET = resolve(
  join(ROOT, "rust"),
  process.env.CARGO_TARGET_DIR ?? "target",
);
const CORE = join(
  CARGO_TARGET,
  "wasm32-unknown-unknown/debug/xpute_example.wasm",
);

const ANSWER = 100;

test("example - a command answered with what the core asked its runtime", async () => {
  const runtime = await boot(
    MANIFEST,
    () => ({
      core: { module: readFileSync(CORE), bytes: MANIFEST.mem_bytes[0] },
      program: [asker_imports({ [Command.ASK]: () => ANSWER })],
      policy: new Timeslice(4),
      submit: () => {},
    }),
    QUIET,
  );
  const asker = asker_exports(runtime);
  runtime.resume();
  assert.deepStrictEqual(await asker.sum(2, 3), 2 + 3 + ANSWER);
});
