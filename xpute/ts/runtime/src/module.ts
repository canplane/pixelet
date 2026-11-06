// @xpute/runtime/module.ts

/**
 * A core built as one WebAssembly module, opened over a memory the runtime
 * reserved (mem/reserve.ts). Its imports are xpute's alone, the memory, the
 * clock and the call, and its entry is `run`; what else it exports, a boot
 * call that answers its front's offset among them, is the program's, on
 * `instance`. Compiling is the caller's.
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
  // A call is answered by whatever the runtime mounted on it says.
  let answer = (_at: u32): i32 => Errno.ENOSYS;
  const instance = new WebAssembly.Instance(module, {
    env: { memory, now, call: (at: u32) => answer(at) },
  });
  const run = instance.exports.run as (deadline_ms: f64) => f64;
  return {
    // The maximum is the initial size, so the buffer is never replaced.
    mem: memory.buffer as ArrayBuffer,
    run,
    answer_with: (a) => {
      answer = a;
    },
    instance,
  };
}
