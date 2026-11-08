// @xpute/runtime/module.ts

/**
 * A core built as one WebAssembly module, opened over a memory the runtime
 * reserved (mem/reserve.ts). The module imports only xpute's three: the
 * memory, the clock and the call. Its entry is `run`. Anything else it
 * exports, such as a boot call that answers its front's offset, belongs to
 * the program and is reached on `instance`. The caller compiles the module.
 */

import type { f64, i32, u32 } from "@xpute/kit/abi/word.ts";
import * as Errno from "@xpute/kit/status/errno.spec.ts";
import type { Core } from "./runtime.ts";

export interface ModuleCore extends Core {
  instance: WebAssembly.Instance;
}

/** `now` is the runtime's clock in milliseconds; `performance.now()` unless the
 * caller keeps its own, as a test does. */
export function open_module(
  module: WebAssembly.Module,
  memory: WebAssembly.Memory,
  now: () => f64 = () => performance.now(),
): ModuleCore {
  let answer = (_at: u32): i32 => Errno.ENOSYS;
  const instance = new WebAssembly.Instance(module, {
    env: { memory, now, call: (at: u32) => answer(at) },
  });
  const run = instance.exports.run as (deadline_ms: f64) => f64;
  return {
    // The memory's maximum is its initial size, so it never grows and its
    // buffer is never replaced.
    mem: memory.buffer as ArrayBuffer,
    run,
    answer_with: (a) => {
      answer = a;
    },
    instance,
  };
}
