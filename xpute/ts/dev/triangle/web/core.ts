// @xpute/triangle/web/core.ts

/**
 * The manifest of the core that draws (rust/dev/triangle), which both the page
 * and the worker import. Build the core first, from rust/:
 *
 *   cargo build -p xpute-triangle --target wasm32-unknown-unknown
 */

import { WebGpu } from "@xpute/gpu/web.ts";
import { type Manifest, Timeslice } from "@xpute/runtime";
import { MACHINE } from "../machine.spec.ts";

/** The major the program gives the GPU's commands, which the core names too. */
export const GPU_MAJOR = 0x10;

export const gpu = new WebGpu();

const CORE =
  "../../../../rust/target/wasm32-unknown-unknown/debug/xpute_triangle.wasm";

export const manifest: Manifest = {
  machine: MACHINE,
  core: () => ({
    core: {
      module: new URL(CORE, import.meta.url),
      bytes: MACHINE.mem_bytes[0],
    },
    after_turn: () => gpu.after_turn(),
    policy: new Timeslice(4),
    submit: () => {},
  }),
};
