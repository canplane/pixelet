// @xpute/runtime-web/worker.ts

/** The worker's end: the guest's module instantiated with a link laid over it,
 * the guest run on it from the program's mounts, and what the runtime tells the
 * page, the guest's stop and its trap, each posted to the page (`tell`), where
 * the UI hears them (page.ts), as the native runtime tells its app through the
 * functions the app sets (xpute-runtime-native listener.rs). Its writer (`log`)
 * is the one place web xpute writes to the console: the log records its host
 * hands it, and the stop and the trap it tells. Each record it writes is told
 * the page as well (`Log`), where a tool that measures the page reads its
 * lines; natively the tool reads the process's log, which xpute-runtime-native
 * log.rs writes. */

import type { i32, u64 } from "@xpute/kit/abi/word.ts";
import { XputeBug, frames } from "@xpute/kit/status/bug.ts";
import {
  ERRNO_ENOTRECOVERABLE,
  ERRNO_EPIPE,
} from "@xpute/kit/status/errno.spec.ts";
import { strerror } from "@xpute/kit/status/error.ts";
import {
  type Handle,
  type Imports,
  type Method,
  Side,
} from "@xpute/link/link.ts";
import { init, type Stop } from "@xpute/host/runtime.ts";
import { type Level, LEVEL_ERROR } from "@xpute/host/abi/log/log.enum.ts";
import { tell_attached } from "./life.ts";
import type { Instantiate, Log, Stopped, Trap } from "./message.ts";
import { attach, type Taker } from "./remote.ts";

interface Exports {
  memory: WebAssembly.Memory;
  link_init(): u64;
  link_boot(): void;
  link_in(): number;
}

/** What stopped the guest as text: an error with its stack. */
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

/** Writes `msg` at `level`, about `context`, as xpute-runtime-native log.rs
 * writes the native runtime's: a log record the host hands the runtime
 * (@xpute/host `Runtime`), and the stop and the trap it tells. The record is
 * told the page too (`Log`), which writes nothing of it. */
function log(level: Level, context: string, msg: string): void {
  console[CONSOLE[level]](`[${context}] ${msg}`);
  tell({ kind: "log", level, context, msg });
}

/** Writes the stop of `context`, the guest or the host, with its message
 * where it has one, then tells the page the guest stopped, the message left
 * out, as xpute-runtime-native native.rs `tell_stop` tells the native app. */
function tell_stop(context: string, stop: Stop): void {
  const { msg, ...report } = stop;
  const { errnum, file, line, n1, n2 } = report;
  log(
    LEVEL_ERROR,
    context,
    `stopped: ${strerror(errnum)} at ${file}:${line} (${n1}, ${n2})${msg === null ? "" : `: ${msg}`}`,
  );
  tell({ kind: "stopped", stop: report });
}

/** The guest's stop, as its host hands it (@xpute/host `mount_stop`). */
function stopped(stop: Stop): void {
  tell_stop("guest", stop);
}

/** Writes the trap, with what stopped it, then tells the page, as
 * xpute-runtime-native link.rs `trap` tells the native app. */
function tell_trap(e: unknown): void {
  const err = line(e);
  log(LEVEL_ERROR, "link", `trapped: ${err}`);
  tell({ kind: "trap", err });
}

/** The host's end of the link over the guest's boxes, and the guest's
 * exports that are no frame, which the runtime enters itself. */
export interface Instance {
  side: Side;
  init(): u64;
  boot(): void;
}

/** Stops `side` for good, where it has not trapped already, and writes and
 * tells `e`, what stopped it, as xpute-runtime-native link.rs `trap` does. */
function trap(side: Side, e: unknown): void {
  if (side.trapped()) return;
  side.trap();
  tell_trap(e);
}

/** Instantiates the guest's `module` and lays the host's end of the link over
 * its boxes (@xpute/link `Side`). The module's memory is its own, sized and
 * grown as the module asks, so the side reads it through the buffer it has at
 * each access. An instantiation that fails is a failure of the host's own
 * before the guest's first entry, told as a trap with its error, and answers
 * null, as the native runtime traps on its own failure before it enters the
 * guest. A trap out of any export stops the guest for good (`trap`), as does
 * a failure of the host's own (`hook`), from before any implementation is
 * mounted: the page hears it, with the error, as a native host tells its
 * trap. The trapped export answers as xpute-runtime-native link.rs `cross_in`
 * does, a request EPIPE, and nothing is thrown on to the host's caller. The
 * guest's import, `link_out`, answers as xpute-runtime-native link.rs
 * `link_out` does: a throw out of the host while it answers is a failure of
 * its own, made a stop, and its call answered ENOTRECOVERABLE, so nothing
 * unwinds through it; once trapped, a frame is not run, EPIPE. The worker
 * lives on after a trap, as the native guest thread does. */
export async function instantiate(
  module: string,
  init: RequestInit | undefined,
): Promise<Instance | null> {
  let side!: Side;
  let fail!: (e: unknown) => void;
  let x: Exports;
  try {
    const { instance } = await WebAssembly.instantiateStreaming(
      fetch(module, init),
      {
        env: {
          link_out: (at: u64): i32 => {
            if (side.trapped()) return ERRNO_EPIPE;
            try {
              return side.recv(at);
            } catch (e) {
              fail(e);
              return ERRNO_ENOTRECOVERABLE;
            }
          },
        },
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
        trap(side, e);
        return trapped;
      }
    };
  side = new Side({
    get mem() {
      return x.memory.buffer;
    },
    in: entry(x.link_in, ERRNO_EPIPE),
  });
  fail = hook(side);
  return {
    side,
    init: entry(x.link_init, 0n),
    boot: entry(x.link_boot, undefined),
  };
}

/** Makes a failure of the host's own a stop, as xpute-runtime-native
 * native.rs `hook` makes a panic one: a throw out of its answer to the
 * guest's import, which `link_out` hands what this answers, and any the
 * worker leaves unhandled or uncaught, as a method answered later throws
 * one. Where the guest has not trapped already, the failure is written and
 * told the page as a stop, with its errno, a bug's or ENOTRECOVERABLE, where
 * it was thrown and its message, a bug's or a throw's own; then the link
 * traps, so that no answer the failed host owed leaves the guest waiting, and
 * the page hears the trap after the stop, as it does natively. A throw after
 * the guest trapped is no failure of the host's. */
function hook(side: Side): (e: unknown) => void {
  const fail = (e: unknown): boolean => {
    if (side.trapped()) return false;
    const stack = e instanceof Error ? (e.stack ?? "") : "";
    const errnum = e instanceof XputeBug ? e.errnum : ERRNO_ENOTRECOVERABLE;
    const [file, line] =
      e instanceof XputeBug ? [e.file, e.line] : (frames(stack)[0] ?? ["", 0]);
    const [n1, n2] = e instanceof XputeBug ? [e.a, e.b] : [0n, 0n];
    const msg =
      e instanceof XputeBug
        ? e.msg
        : e instanceof Error
          ? e.message
          : String(e);
    tell_stop("host", { errnum, file, line, n1, n2, msg });
    trap(side, e);
    return true;
  };
  // A failure the hook writes is not reported again by the browser, as the
  // native hook replaces Rust's.
  addEventListener("error", (ev) => {
    if (fail(ev.error)) ev.preventDefault();
  });
  addEventListener("unhandledrejection", (ev) => {
    if (fail(ev.reason)) ev.preventDefault();
  });
  return fail;
}

/** Enters the guest first, once the host's own objects and what the UI
 * implements are mounted: the guest lays its link, the side reads its layout,
 * and then the guest boots, and may open them, as xpute-runtime-native
 * link.rs `init` does. Where an interface of `imports` is implemented by
 * nothing mounted, under its named handle or by an opener of its resources
 * (`Side.implements`), or where one of `calls` (the bindings' `CALLS`) is
 * among the interfaces the UI implements (`ui`), the link traps, naming each,
 * and the guest is never entered. False where the link trapped meanwhile, the
 * guest's trap or a failure of the host's. */
function enter(
  g: Instance,
  imports: Imports,
  calls: Calls,
  ui: readonly Handle[],
): boolean {
  const { side } = g;
  const name = (h: Handle) => imports.find(([, i]) => i === h)?.[0] ?? `${h}`;
  const unimplemented = imports
    .filter(([, h]) => !side.implements(h))
    .map(([name]) => name);
  if (unimplemented.length > 0) {
    trap(side, `no implementation of ${unimplemented.join(", ")}`);
    return false;
  }
  const over = [...new Set(calls.map(([h]) => h).filter((h) => ui.includes(h)))]
    .map(name)
    .join(", ");
  if (over !== "") {
    trap(side, `a function without async is implemented by the UI: ${over}`);
    return false;
  }
  const at = side.crossed(g.init);
  if (side.trapped()) return false;
  side.set_layout(at);
  side.crossed(g.boot);
  return !side.trapped();
}

/** What mounts the implementations of some of the guest's imports on the
 * host's end, whose boxes read the guest's memory. */
export type Mount = (side: Side) => void;

/** The methods a world's imports answer at once (the bindings' `CALLS`). */
export type Calls = readonly (readonly [Handle, Method])[];

/** What a program starts its guest with: its mounts, in no order, since
 * none depends on another; the interfaces its guest's world imports (the
 * bindings' `IMPORTS`); the methods they hold that answer at once, xpute's
 * and a plugin's among them (the bindings' `CALLS`), which the UI may not
 * implement; and what takes the things the UI gives beside its calls. */
export interface Boot {
  mounts: readonly Mount[];
  imports: Imports;
  calls: Calls;
  taker: Taker;
}

/** Runs the guest on `g`, as xpute-runtime-native native.rs `run` does
 * natively: gives the host what it needs of the runtime (@xpute/host
 * `Runtime`), runs each mount, attaches the UI across the port `ui` names
 * (`attach`), enters the guest, which an import with no implementation and a
 * call the UI implements refuse (`enter`), and tells it the UI attached. */
export function run(
  g: Instance,
  boot: Boot,
  ui: Pick<Instantiate, "port" | "named" | "prefixes">,
): void {
  init({ stopped, log });
  for (const m of boot.mounts) m(g.side);
  attach(ui.port, g.side, ui.named, ui.prefixes, boot.taker);
  const ui_handles = [...ui.named, ...ui.prefixes.map(([h]) => h)];
  enter(g, boot.imports, boot.calls, ui_handles);
  tell_attached(g.side);
}
