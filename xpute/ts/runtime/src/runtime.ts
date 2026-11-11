// @xpute/runtime/runtime.ts

/**
 * A runtime is mounted on a core that its platform opened.
 *
 * - The front is laid before the first turn (io/head.ts).
 * - The connection runs each turn and, as the turn ends, reads what the core
 *   sent, even when the turn trapped.
 * - What the core sends goes to whatever answers its major: xpute's own
 *   handlers below PROGRAM_MAJOR (the stop report, the file device), and the
 *   program's from there up. Each command the UI takes also goes to the UI.
 * - The scheduler runs turns as the policy says (sched/policy.ts).
 */

import type { f64, u32 } from "@xpute/kit/abi/word.ts";
import * as Errno from "@xpute/kit/status/errno.spec.ts";
import { errno_name } from "@xpute/kit/status/errno.spec.ts";
import { InvariantError } from "@xpute/kit/status/error.ts";
import type { Arg } from "@xpute/kit/wire/xtp/mod.ts";
import {
  Command,
  sys_imports,
  type SysAbort,
} from "@xpute/runtime/abi/sys.spec.ts";
import * as fs from "@xpute/runtime/abi/fs.spec.ts";
import { type Front, Head, laid, lay } from "@xpute/runtime/io/head.ts";
import * as frame from "@xpute/runtime/io/frame.ts";
import {
  answer_of,
  Connection,
  encode_args,
  type Heard,
  type Host,
  type OnImport,
} from "@xpute/runtime/io/connection.ts";
import { type Binding, FsServer } from "@xpute/runtime/io/fs.ts";
import type { Grant, SchedulingPolicy } from "@xpute/runtime/sched/policy.ts";
import { Scheduler } from "@xpute/runtime/sched/scheduler.ts";
import { open } from "./platform";
import { say } from "./say.ts";

/** The core as its platform opened it. Its memory never moves or grows.
 * `run` runs a turn and returns when the core wants the next, in ms; xpute
 * names this entry, and the program implements it under that name. */
export interface Core {
  mem: ArrayBuffer;
  run(deadline_ms: f64): f64;
  /** Makes `host` answer what the core receives, sends and calls from here
   * on (xpute-core host.rs); a core that reaches its runtime before then stops
   * with ENOSYS (`UNSET`). */
  host_with(host: Host): void;
}

/** The core gives this report as it aborts: the errno of the broken invariant,
 * where in the source it broke, and two numbers the core gave. */
export interface Abort {
  errnum: number;
  file: string;
  line: u32;
  column: u32;
  a: bigint;
  b: bigint;
}

/** The core's stop report as program code takes it. */
export function abort_of(e: SysAbort): Abort {
  const num = (low: u32, high: u32) => BigInt(low) | (BigInt(high) << 32n);
  return {
    errnum: e.errnum,
    file: e.file,
    line: e.line,
    column: e.column,
    a: num(e.a_low, e.a_high),
    b: num(e.b_low, e.b_high),
  };
}

/** The core's abort, as it is logged, the line the Rust runtime writes
 * (xpute-runtime report.rs `abort_report`): the errno by its name. */
export function abort_report(e: SysAbort): string {
  const r = abort_of(e);
  return `[xpute] stopped: ${errno_name(r.errnum)} at ${r.file}:${r.line}:${r.column} (${r.a}, ${r.b})`;
}

function say_abort(e: SysAbort): void {
  say(abort_report(e));
}

/** This is everything the app decides for its core: the module, where a page
 * loads one; what backs the files, where the manifest asks for them; what runs
 * beside a page's core as each turn returns (`after_turn`); what answers the
 * program's commands (`program`); its scheduling policy, and what the app
 * submits with each turn it grants (`submit`). */
export interface Config<G extends Grant = Grant> {
  core?: ModuleSource;
  fs?: Binding;
  after_turn?(): void;
  /** A turn of the core that takes longer than this many milliseconds is
   * said, as `[xpute] turn took <ms> ms`, the whole milliseconds, as a native
   * runtime says it (xpute-runtime `native::Config::slow_ms`). With none, no
   * turn is timed. */
  slow_ms?: f64;
  program?: Protocol[];
  policy: SchedulingPolicy<G>;
  submit(grant: G, delta_s: f64): void;
}

/** This is what a UI hears of its core: each import on a major its protocols
 * take (sys's among them, for a UI that shows the stop), as their generated
 * `{proto}_imports` read it, and `stop`, that the core has stopped. */
export interface Ui {
  protocols: readonly Protocol[];
  stop(): void;
}

export const QUIET: Ui = { protocols: [], stop() {} };

/** A protocol as the runtime routes it, as its generated `{proto}_imports`
 * makes it: the majors its commands take, and what takes the imports it
 * carries. */
export interface Protocol {
  majors: readonly u32[];
  on: OnImport;
}

/** This is the UI's link to its runtime, beside it or across a port. */
export interface Link {
  /** Sends a command with the next turn. */
  send(cmd: u32, ...args: Arg[]): void;
  /** Sends a command with ACKREQ. The promise resolves with the core's
   * reply, which comes with a later turn, or rejects with the errno the
   * command was refused with (`answer_of`). */
  request(cmd: u32, ...args: Arg[]): Promise<u32>;
  /** This says whether a command's packet fits a frame. */
  fits(cmd: u32, ...args: Arg[]): boolean;
  /** Asks for a turn. */
  wake(): void;
  /** Tells the policy that an input came now. */
  input(): void;
  /** Lets turns run, while the program is shown. */
  resume(): void;
  /** Stops turns, once what was sent before it has reached the core. */
  pause(): void;
  /** Says whether the program is shown. Each change is sent to the core
   * (SYS_RESUME, SYS_SUSPEND), and SYS_SUSPEND reaches it before turns stop,
   * as hiding is the core's last sure moment to save what it holds. */
  visible(visible: boolean): void;
  /** Whether the UI shows the core, as `visible` last said; true until it
   * says otherwise. */
  shown(): boolean;
}

export interface Runtime extends Link {
  core: Core;
  head: Head;
  connection: Connection;
  /** Tells the scheduler of what was submitted on `connection` itself. */
  kick(): void;
}

/** This is the memory of the core last mounted here. */
export let mem: ArrayBuffer;

export function bind_mem(m: ArrayBuffer): void {
  mem = m;
}

/** Mounts the runtime on `core`. The front is laid as `m` places it, unless
 * it was laid already. */
export function mount<G extends Grant>(
  core: Core,
  m: Front & Pick<Manifest, "head_at">,
  cfg: Config<G>,
  ui: Ui,
): Runtime {
  if (!laid(core.mem, m.head_at)) lay(core.mem, m.head_at, m);
  mem = core.mem;
  const head = new Head(core.mem, m.head_at);
  const policy = cfg.policy;

  // The scheduler hears each turn the connection runs, and asks it for each.
  const heard: Heard = {
    on_turn: () => scheduler.on_turn(),
    on_wake: (wake_ms) => scheduler.on_wake(wake_ms),
  };
  const connection = new Connection(
    core.mem,
    head.pkt_bytes,
    (deadline_ms) => {
      const slow = cfg.slow_ms;
      const t0 = slow === undefined ? 0 : performance.now();
      const wake = core.run(deadline_ms);
      if (slow !== undefined) {
        const ms = Math.trunc(performance.now() - t0);
        if (ms > slow) say(`[xpute] turn took ${ms} ms`);
      }
      cfg.after_turn?.();
      return wake;
    },
    heard,
  );
  const scheduler = new Scheduler(connection, policy, (grant, delta_s) => {
    const f = policy.frame?.(grant);
    if (f) {
      const args = [f.refresh_ms, f.paced_ms, f.interactive];
      connection.send(Command.SYS_FRAME, encode_args(args));
    }
    cfg.submit(grant, delta_s);
    connection.send(Command.SYS_TURN, encode_args([Date.now()]));
  });

  const kick = <T>(v: T): T => (scheduler.wake(), v);
  const send = (cmd: u32, ...args: Arg[]) =>
    kick(connection.send(cmd, encode_args(args)));

  const files = cfg.fs
    ? fs.fs_imports(
        new FsServer(cfg.fs, core.mem, (tag, cmd, res, encode) =>
          kick(connection.complete(tag, cmd, res, encode)),
        ),
      )
    : NO_FILES;
  const routes = routes_of([
    sys_imports({
      [Command.SYS_ABORT]: say_abort,
      [Command.SYS_PACE]: (e) => policy.pace?.(e.interval_ms),
    }),
    files,
    ...(cfg.program ?? []),
  ]);
  const ui_routes = routes_of(ui.protocols);
  connection.listen((tag, cmd, f, pkt, named) => {
    const major = frame.major_of(cmd);
    const on = routes.get(major);
    const take = ui_routes.get(major);
    take?.(tag, cmd, f, pkt, named);
    // What the UI alone takes is answered by nothing here.
    return on ? on(tag, cmd, f, pkt, named) : take ? undefined : Errno.ENOSYS;
  });
  connection.onterminated = () => {
    scheduler.pause();
    ui.stop();
  };
  core.host_with(connection.host);

  // Turns run while resumed and shown.
  let resumed = false;
  let shown = true;

  function steer(): void {
    if (resumed && shown) scheduler.resume();
    else scheduler.pause();
  }

  // The core hears a runtime attach before anything else.
  connection.send(Command.SYS_ATTACH);
  return {
    core,
    head,
    connection,
    shown: () => shown,
    send,
    request: (cmd, ...args) =>
      kick(connection.request(cmd, encode_args(args))).then(answer_of),
    fits: (_cmd, ...args) => {
      const pkt = encode_args(args)(connection);
      return pkt === null || connection.fits(pkt);
    },
    kick: () => scheduler.wake(),
    wake: () => scheduler.wake(),
    input: () => policy.input?.(performance.now()),
    resume: () => ((resumed = true), steer()),
    pause: () => ((resumed = false), steer()),
    visible(visible) {
      if (visible === shown) return;
      shown = visible;
      send(visible ? Command.SYS_RESUME : Command.SYS_SUSPEND);
      steer();
    },
  };
}

/** What takes each major's imports, from the protocols that carry them. */
export function routes_of(protocols: readonly Protocol[]): Map<u32, OnImport> {
  const routes = new Map<u32, OnImport>();
  for (const p of protocols) for (const m of p.majors) routes.set(m, p.on);
  return routes;
}

/** The file device of a manifest that asks for none: each open and close is
 * answered ENODEV (abi/fs.json). */
export const NO_FILES: Protocol = {
  majors: [fs.Major.FS],
  on: () => Errno.ENODEV,
};

/** This is a core's manifest, as its generated `MANIFEST` gives it: the memory
 * sizes it allows, the front and where its header is, and the devices the core
 * asks for. */
export interface Manifest extends Front {
  mem_bytes: readonly u32[];
  region_bytes: u32;
  head_at: u32;
  fs: boolean;
}

/** This says where a page's core comes from: its module, fetched from a URL or
 * given as bytes, and the size of its memory in bytes, which its manifest must
 * allow. */
export interface ModuleSource {
  module: URL | BufferSource;
  bytes: u32;
  init?: RequestInit;
}

/** Opens the core `manifest` declares on this build's platform and mounts the
 * runtime on it, with the config `core` makes where the core runs; what the
 * core tells a UI goes to `ui`. The core takes `now` as its clock, which counts
 * as the UI's `performance.now()` does. It refuses with EINVAL a config without
 * `fs` for a manifest that asks for files, and a config with `fs` for a
 * manifest that does not. */
export async function boot<G extends Grant>(
  manifest: Manifest,
  core: () => Config<G> | Promise<Config<G>>,
  ui: Ui,
  now: () => f64 = () => performance.now(),
): Promise<Runtime> {
  const cfg = await core();
  if (manifest.fs !== (cfg.fs !== undefined)) {
    throw new InvariantError(Errno.EINVAL);
  }
  return mount(await open(manifest, cfg.core, now), manifest, cfg, ui);
}
