// @xpute/runtime/platform.ts

/**
 * Opens a page's core: its module, compiled from its bytes and opened over
 * memory the runtime reserves.
 */

import * as Errno from "@xpute/kit/status/errno.spec.ts";
import { InvariantError } from "@xpute/kit/status/error.ts";
import type { f64 } from "@xpute/kit/abi/word.ts";
import type { Core, Manifest, ModuleSource } from "./runtime.ts";
import { reserve_mem } from "./reserve.ts";
import { open_module } from "./module.ts";

/** Throws EINVAL without a source, or for a memory size the manifest does not
 * allow. The module's clock, `env.now`, is `now`. The module is compiled from
 * its bytes rather than streamed, so that a `file:` URL works too: streaming
 * compilation refuses a response that is not `application/wasm`. */
export async function open(
  manifest: Manifest,
  source: ModuleSource | undefined,
  now: () => f64,
): Promise<Core> {
  if (!source || !manifest.mem_bytes.includes(source.bytes)) {
    throw new InvariantError(Errno.EINVAL);
  }
  const region = manifest.region_bytes;
  const mem = reserve_mem([source.bytes / region], region);
  const wasm =
    source.module instanceof URL
      ? await (await fetch(source.module, source.init)).arrayBuffer()
      : source.module;
  return open_module(await WebAssembly.compile(wasm), mem, now);
}
