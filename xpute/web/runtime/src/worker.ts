// @xpute/web-runtime/worker.ts

/** The worker's end: the guest's module instantiated with a link laid over
 * it, and the worker's console carried to the page. */

import { Link } from "@xpute/link";
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
 * grown whole to `mem_bytes` before its first entry, and never after. An
 * instantiation that fails tells its error and answers null. A trap out of any export stops the guest
 * for good: the page hears it, with the error, and the worker closes, so
 * nothing enters the guest again. */
export async function instantiate(
  module: string,
  init: RequestInit | undefined,
  mem_bytes: number,
  tell: (m: InstantiateError | Trap) => void,
): Promise<Link | null> {
  let link!: Link;
  let x: Exports;
  try {
    const { instance } = await WebAssembly.instantiateStreaming(
      fetch(module, init),
      {
        env: Link.imports(() => link),
      },
    );
    x = instance.exports as unknown as Exports;
    x.memory.grow(Math.ceil((mem_bytes - x.memory.buffer.byteLength) / 65536));
  } catch (e) {
    tell({ kind: "instantiate_error", err: line(e) });
    return null;
  }
  const entry =
    <T>(f: () => T) =>
    (): T => {
      try {
        return f();
      } catch (e) {
        tell({ kind: "trap", err: line(e) });
        close();
        throw e;
      }
    };
  link = new Link({
    get mem() {
      return x.memory.buffer;
    },
    init: entry(x.link_init),
    boot: entry(x.link_boot),
    in: entry(x.link_in),
  });
  return link;
}
