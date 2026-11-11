// @xpute/runtime/module.ts

/**
 * This is a core built as one WebAssembly module, opened over a memory the
 * runtime reserved (reserve.ts). The module imports only xpute's: the memory,
 * the clock, and the host it receives, sends and calls through. Its entry is `run`. Anything else it exports,
 * such as a boot call that answers its front's offset, belongs to the program
 * and is reached on `instance`. The caller compiles the module.
 */

import type { f64, i32, u32 } from "@xpute/kit/abi/word.ts";
import { type Host, UNSET } from "./io/connection.ts";
import type { Core } from "./runtime.ts";

export interface ModuleCore extends Core {
  instance: WebAssembly.Instance;
}

/** `now` is the runtime's clock in milliseconds; `performance.now()` unless the
 * caller keeps its own, as a worker does to count as its UI's clock, or as a
 * test does. */
export function open_module(
  module: WebAssembly.Module,
  memory: WebAssembly.Memory,
  now: () => f64 = () => performance.now(),
): ModuleCore {
  let host: Host = UNSET;
  const instance = new WebAssembly.Instance(module, {
    env: {
      memory,
      now,
      receive: (at: u32, cap: u32): i32 => host.receive(at, cap),
      send: (at: u32, named_at: u32, named_len: u32) =>
        host.send(at, named_at, named_len),
      call: (at: u32): i32 => host.call(at),
    },
  });
  const run = instance.exports.run as (deadline_ms: f64) => f64;
  return {
    // The memory's maximum is its initial size, so it never grows and its
    // buffer is never replaced.
    mem: memory.buffer as ArrayBuffer,
    run,
    host_with: (h) => {
      host = h;
    },
    instance,
  };
}
