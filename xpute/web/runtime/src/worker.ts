// @xpute/web-runtime/worker.ts

/** The worker's end: the guest's module instantiated with a link laid over
 * it, the guest started on it from the program's servers, and the worker's
 * console carried to the page. */

import { Bug, frames } from "@xpute/kit/status/bug.ts";
import { ERRNO_ENOTRECOVERABLE, ERRNO_EPIPE } from "@xpute/kit/status/errno.spec.ts";
import { strerror } from "@xpute/kit/status/error.ts";
import { type Imports, set_hook, Side } from "@xpute/link";
import type { Later } from "@xpute/link/remote.ts";
import { tell_attached } from "./life.ts";
import type { Instantiate, InstantiateError, Log, Stopped, Trap } from "./message.ts";
import { serve, type Taker } from "./remote.ts";

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
 * host's own (`hook`), from before any server is mounted: the page hears it,
 * with the error, and the worker closes, once the current turn is over, as a
 * native host posts its trap. The trapped export answers as
 * xpute-native-runtime link.rs `cross_in` does, a request EPIPE, and nothing
 * is thrown on to the host's caller. */
export async function instantiate(
  module: string,
  init: RequestInit | undefined,
  mem_sz: number,
  tell: (m: InstantiateError | Trap | Stopped) => void,
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
  hook(link, tell);
  return link;
}

/** Makes a failure of the host's own a stop, as xpute-native-runtime
 * native.rs `hook` makes a panic one: a throw out of its answer to the
 * guest's import, which the link hands it (@xpute/link `set_hook`), and any
 * the worker leaves unhandled or uncaught, as a method answered later throws
 * one. The link's `trap` stops the guest, so that no answer the failed host
 * owed leaves the guest waiting, and where it did just now, the failure is
 * logged and told the page as a stop, with its errno, a bug's or
 * ENOTRECOVERABLE, and where it was thrown, before the page hears the trap.
 * A throw after the guest trapped is no failure of the host's. */
function hook(link: Side, tell: (m: Stopped) => void): void {
  const fail = (e: unknown) => {
    if (!link.trap(e)) return;
    const stack = e instanceof Error ? (e.stack ?? "") : "";
    const errnum = e instanceof Bug ? e.errnum : ERRNO_ENOTRECOVERABLE;
    const [file, line] = e instanceof Bug ? [e.file, e.line] : (frames(stack)[0] ?? ["", 0]);
    console.error(`[host] stopped: ${strerror(errnum)} at ${file}:${line}`);
    tell({ kind: "stopped", stop: { errnum, file, line, n1: 0n, n2: 0n } });
  };
  set_hook(fail);
  addEventListener("error", (ev) => fail(ev.error));
  addEventListener("unhandledrejection", (ev) => fail(ev.reason));
}

/** What serves the guest some of its imports: it mounts them on the link. */
export type Server = (link: Side) => void;

/** What a program starts its guest with: its servers, in no order, since
 * none depends on another; the interfaces its guest's world imports (the
 * bindings' `IMPORTS`); and the UI's side, the methods of
 * the UI's named interfaces that answer later (the bindings' `LATER`) and
 * what takes the things the UI gives beside the link. */
export interface Boot {
  servers: readonly Server[];
  imports: Imports;
  later: Later;
  taker: Taker;
}

/** Starts the guest on `link`, as xpute-native-runtime native.rs `run`
 * does on a device: mounts each server, serves the link to the UI across
 * the port `ui` names (`serve`), enters the guest, which an import no server
 * mounts refuses (`Side.init`), and tells it the UI attached. */
export function start(
  link: Side,
  boot: Boot,
  ui: Pick<Instantiate, "port" | "named" | "prefixes">,
): void {
  for (const s of boot.servers) s(link);
  serve(ui.port, link, ui.named, ui.prefixes, boot.later, boot.taker);
  link.init(boot.imports);
  tell_attached(link);
}
