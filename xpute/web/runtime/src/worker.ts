// @xpute/web-runtime/worker.ts

/** The worker's end: the guest's module instantiated with a link laid over
 * it, and the worker's console carried to the page. */

import { ERRNO_EPIPE } from "@xpute/kit/status/errno.spec.ts";
import { Side } from "@xpute/link";
import type { InstantiateError, Log, Trap } from "./message.ts";

interface Exports {
  memory: WebAssembly.Memory;
  link_init(): number;
  link_boot(): void;
  link_in(): number;
}

/** A console argument as text: an error with its stack. */
function line(a: unknown): string {
  if (a instanceof Error) return a.stack ?? a.message;
  try {
    return String(a);
  } catch {
    return Object.prototype.toString.call(a);
  }
}

/** What the worker writes to its console goes to the page's lines too, where
 * a tool driving the page reads them. */
export function forward_console(tell: (m: Log) => void): void {
  for (const level of ["log", "info", "warn", "error"] as const) {
    const write = console[level].bind(console);
    console[level] = (...args: unknown[]) => {
      write(...args);
      tell({ kind: "log", text: args.map(line).join(" ") });
    };
  }
}

/** Instantiates the guest's `module` and lays a link over it: its memory
 * grown whole to `mem_sz` before its first entry, and never after. An
 * instantiation that fails tells its error and answers null. A trap out of
 * any export stops the guest for good (`Side.trap`), as does a failure of the
 * host's own: the page hears it, with the error, and the worker closes, once
 * the current turn is over, as a native host posts its trap. The trapped
 * export answers as xpute-native-runtime link.rs `cross_in` does, a request
 * EPIPE, and nothing is thrown on to the host's caller. */
export async function instantiate(
  module: string,
  init: RequestInit | undefined,
  mem_sz: number,
  tell: (m: InstantiateError | Trap) => void,
): Promise<Side | null> {
  let link!: Side;
  let x: Exports;
  try {
    const { instance } = await WebAssembly.instantiateStreaming(
      fetch(module, init),
      {
        env: Side.imports(() => link),
      },
    );
    x = instance.exports as unknown as Exports;
    x.memory.grow(Math.ceil((mem_sz - x.memory.buffer.byteLength) / 65536));
  } catch (e) {
    tell({ kind: "instantiate_error", err: line(e) });
    return null;
  }
  const entry =
    <T>(f: () => T, trapped: T) =>
    (): T => {
      try {
        return f();
      } catch (e) {
        link.trap(e);
        return trapped;
      }
    };
  link = new Side({
    get mem() {
      return x.memory.buffer;
    },
    init: entry(x.link_init, 0),
    boot: entry(x.link_boot, undefined),
    in: entry(x.link_in, ERRNO_EPIPE),
    trap: (e) =>
      queueMicrotask(() => {
        tell({ kind: "trap", err: line(e) });
        close();
      }),
  });
  return link;
}
