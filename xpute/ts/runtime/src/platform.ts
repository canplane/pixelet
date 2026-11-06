// @xpute/runtime/platform.ts

/**
 * A page's core: its module, compiled from its bytes, opened over memory the
 * runtime reserves. A native app's bundler takes platform.native.ts in its
 * place.
 */

import * as Errno from "@xpute/kit/status/errno.spec.ts";
import { InvariantError } from "@xpute/kit/status/error.ts";
import type { Core, Machine, ModuleSource } from "./runtime.ts";
import { reserve_mem } from "./mem/reserve.ts";
import { open_module } from "./module.ts";

/** EINVAL without a source, or with memory of a size its machine does not
 * allow. Compiling from bytes reads a `file:` URL too, which streaming
 * refuses for want of an `application/wasm` response. */
export async function open(
  machine: Machine,
  source: ModuleSource | undefined,
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
  return open_module(await WebAssembly.compile(wasm), mem);
}
