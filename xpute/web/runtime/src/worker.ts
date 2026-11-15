// @xpute/web-runtime/worker.ts

/** The worker's end: the guest's module instantiated with a link laid over
 * it, the guest run on it from the program's servers, and what the runtime
 * tells the page, the worker's console, the guest's stop and its trap, each
 * posted to the page (`tell`), where the UI hears them (page.ts), as a
 * device's runtime tells its app through the functions the app sets
 * (xpute-native-runtime listener.rs). Its writer (`log`) is the one place a
 * page's xpute writes to the console: the log records its host hands it,
 * and the stop and the trap it tells. */

import type { u64 } from "@xpute/kit/abi/word.ts";
import { XputeBug, frames } from "@xpute/kit/status/bug.ts";
import {
  ERRNO_ENOTRECOVERABLE,
  ERRNO_EPIPE,
} from "@xpute/kit/status/errno.spec.ts";
import { strerror } from "@xpute/kit/status/error.ts";
import { type Imports, set_hook, Side } from "@xpute/link";
import type { Later } from "@xpute/link/remote.ts";
import { init, type Stop } from "@xpute/web-host";
import { type Level, LEVEL_ERROR } from "@xpute/web-host/abi/log/log.enum.ts";
import { tell_attached } from "./life.ts";
import type { Instantiate, Log, Stopped, Trap } from "./message.ts";
import { serve, type Taker } from "./remote.ts";

interface Exports {
  memory: WebAssembly.Memory;
  link_init(): u64;
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

/** Posts `m` to the page, whose worker this is. */
function tell(m: Trap | Stopped | Log): void {
  postMessage(m);
}

/** The console's method each level is written by, in `Level`'s order. */
const CONSOLE = ["debug", "debug", "info", "warn", "error", "error"] as const;

/** Writes `message` at `level`, about `context`, as xpute-native-runtime
 * log.rs writes a device's: a log record the host hands the runtime
 * (@xpute/web-host `Runtime`), and the stop and the trap it tells. */
function log(level: Level, context: string, message: string): void {
  console[CONSOLE[level]](`[${context}] ${message}`);
}

/** Writes the stop of `context`, the guest or the host, with its message
 * where it has one, then tells the page the guest stopped, the message left
 * out, as xpute-native-runtime native.rs `tell_stop` tells a device's app. */
function tell_stop(context: string, stop: Stop): void {
  const { message, ...report } = stop;
  const { errnum, file, line, n1, n2 } = report;
  log(
    LEVEL_ERROR,
    context,
    `stopped: ${strerror(errnum)} at ${file}:${line} (${n1}, ${n2})${message === null ? "" : `: ${message}`}`,
  );
  tell({ kind: "stopped", stop: report });
}

/** The guest's stop, as its host hands it (@xpute/web-host `serve_stop`). */
function stopped(stop: Stop): void {
  tell_stop("guest", stop);
}

/** Writes the trap, with what stopped it, then tells the page, as
 * xpute-native-runtime link.rs `trap` tells a device's app. */
function tell_trap(e: unknown): void {
  const err = line(e);
  log(LEVEL_ERROR, "link", `trapped: ${err}`);
  tell({ kind: "trap", err });
}

/** What the worker writes to its console goes to the page's lines too, where
 * a tool driving the page reads them. */
export function forward_console(): void {
  for (const level of ["log", "info", "warn", "error"] as const) {
    const write = console[level].bind(console);
    console[level] = (...args: unknown[]) => {
      write(...args);
      tell({ kind: "log", text: args.map(line).join(" ") });
    };
  }
}

/** Instantiates the guest's `module` and lays a link over it. The module's
 * memory is its own, sized and grown as the module asks, so the link reads
 * it through the buffer it has at each access (`mem`). An instantiation
 * that fails is a failure of the host's own before the guest's first entry,
 * told as a trap with its error, and answers null, as a device's runtime
 * traps on its own failure before it enters the guest. A trap out of
 * any export stops the guest for good (`Side.trap`), as does a failure of the
 * host's own (`hook`), from before any server is mounted: the page hears it,
 * with the error, as a native host tells its trap. The trapped export
 * answers as xpute-native-runtime link.rs `cross_in` does, a request EPIPE,
 * and nothing is thrown on to the host's caller. The worker lives on after
 * a trap, as a device's guest thread does. */
export async function instantiate(
  module: string,
  init: RequestInit | undefined,
): Promise<Side | null> {
  let side!: Side;
  let x: Exports;
  try {
    const { instance } = await WebAssembly.instantiateStreaming(
      fetch(module, init),
      {
        env: Side.imports(() => side),
      },
    );
    x = instance.exports as unknown as Exports;
  } catch (e) {
    tell_trap(e);
    return null;
  }
  const entry =
    <T>(f: () => T, trapped: T) =>
    (): T => {
      try {
        return f();
      } catch (e) {
        side.trap(e);
        return trapped;
      }
    };
  side = new Side({
    get mem() {
      return x.memory.buffer;
    },
    init: entry(x.link_init, 0n),
    boot: entry(x.link_boot, undefined),
    in: entry(x.link_in, ERRNO_EPIPE),
    trap: tell_trap,
  });
  hook(side);
  return side;
}

/** Makes a failure of the host's own a stop, as xpute-native-runtime
 * native.rs `hook` makes a panic one: a throw out of its answer to the
 * guest's import, which the link hands it (@xpute/link `set_hook`), and any
 * the worker leaves unhandled or uncaught, as a method answered later throws
 * one. Where the guest has not trapped already, the failure is written and
 * told the page as a stop, with its errno, a bug's or ENOTRECOVERABLE, where
 * it was thrown and a throw's own message; then the link's `trap` stops the guest, so that no
 * answer the failed host owed leaves the guest waiting, and the page hears
 * the trap after the stop, as on a device. A throw after the guest trapped
 * is no failure of the host's. */
function hook(side: Side): void {
  const fail = (e: unknown) => {
    if (side.trapped()) return;
    const stack = e instanceof Error ? (e.stack ?? "") : "";
    const errnum = e instanceof XputeBug ? e.errnum : ERRNO_ENOTRECOVERABLE;
    const [file, line] =
      e instanceof XputeBug ? [e.file, e.line] : (frames(stack)[0] ?? ["", 0]);
    const message =
      e instanceof XputeBug ? null : e instanceof Error ? e.message : String(e);
    tell_stop("host", { errnum, file, line, n1: 0n, n2: 0n, message });
    side.trap(e);
  };
  set_hook(fail);
  addEventListener("error", (ev) => fail(ev.error));
  addEventListener("unhandledrejection", (ev) => fail(ev.reason));
}

/** What serves the guest some of its imports: it mounts them on the link. */
export type Server = (side: Side) => void;

/** What a program starts its guest with: its servers, in no order, since
 * none depends on another; the interfaces its guest's world imports (the
 * bindings' `IMPORTS`); the methods of the program's own named interfaces
 * among them that answer later (the bindings' `LATER`), which tells the
 * runtime which of the UI's methods answer later; and what takes the things
 * the UI gives beside the link. */
export interface Boot {
  servers: readonly Server[];
  imports: Imports;
  later: Later;
  taker: Taker;
}

/** Runs the guest on `side`, as xpute-native-runtime native.rs `run`
 * does on a device: gives the host what it needs of the runtime
 * (@xpute/web-host `Runtime`), mounts each server, serves the link to the UI
 * across the port `ui` names (`serve`), enters the guest, which an import no
 * server mounts refuses (`Side.init`), and tells it the UI attached. */
export function run(
  side: Side,
  boot: Boot,
  ui: Pick<Instantiate, "port" | "named" | "prefixes">,
): void {
  init({ stopped, log });
  for (const s of boot.servers) s(side);
  serve(ui.port, side, ui.named, ui.prefixes, boot.later, boot.taker);
  side.init(boot.imports);
  tell_attached(side);
}
