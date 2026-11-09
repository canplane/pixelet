// @xpute/runtime/platform.ts

/**
 * Opens a page's core: its module, compiled from its bytes and opened over
 * memory the runtime reserves. A native app's bundler takes
 * platform.native.ts in this file's place.
 */

import * as Errno from "@xpute/kit/status/errno.spec.ts";
import { InvariantError } from "@xpute/kit/status/error.ts";
import type { f64 } from "@xpute/kit/abi/word.ts";
import type { Core, Machine, ModuleSource } from "./runtime.ts";
import { reserve_mem } from "./reserve.ts";
import { open_module } from "./module.ts";

/** Hands `run` the display's next animation frame and its time. */
export function next_frame(run: (now: f64) => void): void {
  requestAnimationFrame(run);
}

/** Throws EINVAL without a source, or for a memory size the machine does not
 * allow. The module's clock, `env.now`, is `now`. The module is compiled from
 * its bytes rather than streamed, so that a `file:` URL works too: streaming
 * compilation refuses a response that is not `application/wasm`. */
export async function open(
  machine: Machine,
  source: ModuleSource | undefined,
  now: () => f64,
): Promise<Core> {
  if (!source || !machine.mem_bytes.includes(source.bytes)) {
    throw new InvariantError(Errno.EINVAL);
  }
  const region = machine.region_bytes;
  const mem = reserve_mem([source.bytes / region], region);
  const wasm =
    source.module instanceof URL
      ? await (await fetch(source.module, source.init)).arrayBuffer()
      : source.module;
  return open_module(await WebAssembly.compile(wasm), mem, now);
}
