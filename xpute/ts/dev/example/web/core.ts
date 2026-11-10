// @xpute/example/web/core.ts

/**
 * This is the manifest of the smallest core (rust/dev/example), which both the page
 * and the worker that runs the core import. It names the core's module and
 * answers ASK with ANSWER. Build the core first, from rust/:
 *
 *   cargo build -p xpute-example --target wasm32-unknown-unknown
 */

import { type Manifest, Timeslice } from "@xpute/runtime";
import { asker_imports, Command } from "../asker.spec.ts";
import { MACHINE } from "../machine.spec.ts";

export const ANSWER = 100;

const CORE =
  "../../../../rust/target/wasm32-unknown-unknown/debug/xpute_example.wasm";

export const manifest: Manifest = {
  machine: MACHINE,
  core: () => ({
    core: {
      module: new URL(CORE, import.meta.url),
      bytes: MACHINE.mem_bytes[0],
    },
    program: asker_imports({ [Command.ASK]: () => ANSWER }),
    policy: new Timeslice(4),
    submit: () => {},
  }),
};
