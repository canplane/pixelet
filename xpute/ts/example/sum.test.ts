// @xpute/example/sum.test.ts

/**
 * The smallest runtime, beside the smallest core (rust/example): booted as the
 * core's machine declares (machine.spec.ts), its module read from disk and
 * opened over a memory of the one size the machine allows, a policy for a
 * runtime with no display, and its one protocol (asker.spec.ts) as the
 * program's handler, which answers the core's call; then one command sent
 * and its reply read. Build the core first (`make build`).
 */

import { test } from "vitest";
import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import { join, resolve } from "node:path";
import { fileURLToPath } from "node:url";

import { boot, QUIET, Timeslice } from "@xpute/runtime";
import { asker_imports, asker_exports } from "./asker.spec.ts";
import { MACHINE } from "./machine.spec.ts";

const ROOT = fileURLToPath(new URL("../../", import.meta.url));
/** A relative CARGO_TARGET_DIR is cargo's, which the root's tasks run from rust/. */
const CARGO_TARGET = resolve(
  join(ROOT, "rust"),
  process.env.CARGO_TARGET_DIR ?? "target",
);
const CORE = join(
  CARGO_TARGET,
  "wasm32-unknown-unknown/debug/xpute_example.wasm",
);

/** The runtime's answer to ASK, which the core adds. */
const ANSWER = 100;

test("example - a command answered with what the core asked its runtime", async () => {
  const runtime = await boot(
    {
      machine: MACHINE,
      core: () => ({
        core: { module: readFileSync(CORE), bytes: MACHINE.mem_bytes[0] },
        program: asker_imports({ ask: () => ANSWER }),
        policy: new Timeslice(4),
        submit: () => {},
      }),
    },
    QUIET,
  );
  assert.deepStrictEqual(asker_exports(runtime).sum(2, 3), 2 + 3 + ANSWER);
});
