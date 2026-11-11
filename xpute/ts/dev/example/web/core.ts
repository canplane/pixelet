// @xpute/example/web/core.ts

/**
 * This is how the smallest core (rust/dev/example) is configured where it
 * runs, which the worker that runs the core imports, and the page for ANSWER.
 * It names the core's module and answers ASK with ANSWER. Build the core first, from rust/:
 *
 *   cargo build -p xpute-example --target wasm32-unknown-unknown
 */

import { type Config, Timeslice } from "@xpute/runtime";
import { asker_imports, Command } from "../asker.spec.ts";
import { MANIFEST } from "../manifest.spec.ts";

export const ANSWER = 100;

const CORE =
  "../../../../rust/target/wasm32-unknown-unknown/debug/xpute_example.wasm";

export const core = (): Config => ({
  core: {
    module: new URL(CORE, import.meta.url),
    bytes: MANIFEST.mem_bytes[0],
  },
  program: [asker_imports({ [Command.ASK]: () => ANSWER })],
  policy: new Timeslice(4),
  submit: () => {},
});
