// @xpute/example/web/core.ts

/**
 * The smallest core's manifest (rust/example), read by the page and by the
 * worker that runs the core: its module, and ASK answered with ANSWER. Build
 * the core first (`make build`).
 */

import { type Manifest, Timeslice } from "@xpute/runtime";
import { asker_imports } from "../asker.spec.ts";
import { MACHINE } from "../machine.spec.ts";

export const ANSWER = 100;

const CORE =
  "../../../rust/target/wasm32-unknown-unknown/debug/xpute_example.wasm";

export const manifest: Manifest = {
  machine: MACHINE,
  core: () => ({
    core: {
      module: new URL(CORE, import.meta.url),
      bytes: MACHINE.mem_bytes[0],
    },
    program: asker_imports({ ask: () => ANSWER }),
    policy: new Timeslice(4),
    submit: () => {},
  }),
};
