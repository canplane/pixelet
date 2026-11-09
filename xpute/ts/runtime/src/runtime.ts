// @xpute/runtime/runtime.ts

/**
 * A runtime is mounted on a core that its platform opened.
 *
 * - The front is laid before the first turn (ipc/head.ts).
 * - The doorbell rings each turn and, as the turn ends, reads what the core
 *   left, even when the turn trapped.
 * - What the core posts goes to whatever answers its major: xpute's own
 *   handlers below PROGRAM_MAJOR (the stop report, the file device), and the
 *   program's from there up. Each command the UI takes also goes to the UI.
 * - The scheduler runs turns as the policy says (sched/policy.ts).
 */

import type { f64, i32, u32 } from "@xpute/kit/abi/word.ts";
import type { U8Array } from "@xpute/kit/abi/array.ts";
import * as Errno from "@xpute/kit/status/errno.spec.ts";
import { strerror } from "@xpute/kit/status/errno.spec.ts";
import { InvariantError } from "@xpute/kit/status/error.ts";
import type { Arg, BranchCursor } from "@xpute/kit/wire/xtp/mod.ts";
import {
  Command,
  Major,
  read,
  sys_imports,
  type SysAbort,
} from "@xpute/runtime/abi/sys.spec.ts";
import { Command as GpuCommand } from "@xpute/runtime/abi/gpu.spec.ts";
import * as fs from "@xpute/runtime/abi/fs.spec.ts";
import { type Front, Head, laid, lay } from "@xpute/runtime/ipc/head.ts";
import * as frame from "@xpute/runtime/ipc/frame.ts";
import {
  Doorbell,
  encode_args,
  type OnImport,
} from "@xpute/runtime/ipc/doorbell.ts";
import { type Fs, FsServer } from "@xpute/runtime/ipc/fs.ts";
import type { Grant, SchedulingPolicy } from "@xpute/runtime/sched/policy.ts";
import { Scheduler } from "@xpute/runtime/sched/scheduler.ts";
import { open } from "./platform";

/** The core as its platform opened it. Its memory never moves or grows.
 * `run` runs a turn and returns when the core wants the next, in ms; xpute
 * names this entry, as a C runtime names `main`. A core that runs on its own
 * thread also gives that thread. */
export interface Core {
  mem: ArrayBuffer;
  run(deadline_ms: f64): f64;
  /** Makes `answer` answer each call from here on; until then every call
   * fails with ENOSYS. */
  answer_with(answer: (at: u32) => i32): void;
  /** This is a native core's thread, which begins its own turns at the
   * display's frames and sleeps otherwise. `wake` asks for a turn at the next
   * frame, and `input` says an input came now. `listen` registers `heard`,
   * which is told when the core has posted or stopped, and whether it has
   * stopped. `pause` stops its turns, and `resume` lets them run again. A
   * page's core has no thread: it runs only inside `run`. */
  thread?: {
    wake(): void;
    input(): void;
    pause(): void;
    resume(): void;
    listen(heard: (stopped: boolean) => void): void;
  };
}

/** This is what a core asks of its GPU (its machine's `gpu`), by WebGPU's
 * names. */
export interface GpuAsk {
  power: string;
  features: { required: readonly string[]; optional: readonly string[] };
}

/** This is a platform's GPU. `grant` opens a device as the core asks.
 * `after_turn` runs the core's stream as each turn returns, and whatever it
 * throws ends the core. `watch` reports when a device answers and whether there
 * is one. `bind` and `unbind` lend it the canvases it draws and take them back.
 * A native GPU binds its views itself, so it takes no canvas. */
export interface Gpu {
  grant(to: GpuGrant): void;
  after_turn(): void;
  watch(report: (state: GpuState) => void): void;
  bind?(handle: u32, canvas: Canvas): void;
  unbind?(handle: u32): void;
}

export type GpuState =
  "present" | "waiting" | "missing" | "lost" | "unsupported" | "insecure";

/** This is a page's canvas, or one transferred to the worker its runtime runs
 * in. */
export type Canvas = HTMLCanvasElement | OffscreenCanvas;

/** The GPU is granted the core's memory, the offset where the core's stream
 * begins, and what the core asks. `ready` tells the core a device answered
 * (GPU_READY), `wake` asks for a turn, and `shown` says whether the UI shows
 * the core (`Link.visible`). */
export interface GpuGrant {
  mem: ArrayBuffer;
  at: u32;
  ask: GpuAsk;
  ready(): void;
  wake(): void;
  shown(): boolean;
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

export function read_abort(f: BranchCursor): Abort {
  return abort_of(read[Command.SYS_ABORT](f));
}

function abort_of(e: SysAbort): Abort {
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

function log_abort(r: Abort): void {
  console.error(
    `[xpute] stopped: ${strerror(r.errnum)} at ${r.file}:${r.line}:${r.column} (${r.a}, ${r.b})`,
  );
}

/** This is everything the app decides for its core: the module, where a page
 * loads one; what backs the files and which GPU is granted, where the machine
 * asks for them; what answers the program's commands (`program`); the
 * scheduling policy, and what the app submits with each turn it grants
 * (`submit`); and the memory an import names, as the protocol's generated
 * `<name>_mem` reads it (`mem_of`). */
export interface Config<G extends Grant = Grant> {
  core?: ModuleSource;
  fs?: Fs;
  gpu?: Gpu;
  program?: OnImport;
  /** A core on its own thread paces itself and takes neither of these. */
  policy: SchedulingPolicy<G>;
  submit(grant: G, delta_s: f64): void;
  mem_of?(cmd: u32, f: BranchCursor | null): [u32, u32] | null;
}

/** This is what a UI hears of its core. `on` is handed each import whose
 * command is in `cmds` (SYS_ABORT among them, for a UI that shows the stop),
 * with `named`, the memory the import names, and `pkt`, its packet. `gpu` hears
 * the GPU's state, and `stop` hears that the core has stopped. */
export interface Ui {
  cmds: readonly u32[];
  on(
    tag: u32,
    cmd: u32,
    f: BranchCursor | null,
    named: U8Array | null,
    pkt: U8Array | null,
  ): void;
  gpu(state: GpuState): void;
  stop(): void;
}

export const QUIET: Ui = { cmds: [], on() {}, gpu() {}, stop() {} };

/** Each command of a protocol, by its number, and the fields its packet takes
 * in order. */
export type FieldTable = Record<u32, Arg[]>;

/** A `Link` as a protocol's generated table types it. Each command goes by the
 * one way its specification names, and only a number crosses at run time. */
export interface Protocol<
  S extends FieldTable,
  R extends FieldTable,
  C extends FieldTable,
> {
  send<K extends keyof S & u32>(cmd: K, ...args: S[K]): void;
  request<K extends keyof R & u32>(cmd: K, ...args: R[K]): Promise<i32>;
  call<K extends keyof C & u32>(cmd: K, ...args: C[K]): i32;
  fits<K extends keyof S & u32>(cmd: K, ...args: S[K]): boolean;
}

/** This is the UI's link to its runtime, beside it or across a port. */
export interface Link {
  /** Sends a command with the next turn. */
  send(cmd: u32, ...args: Arg[]): void;
  /** Sends a command with ACKREQ. The promise settles with the core's
   * reply, which comes with a later turn. */
  request(cmd: u32, ...args: Arg[]): Promise<i32>;
  /** Sends a command and rings the core at once, again and again until it
   * answers, whether turns are paused or not. */
  call(cmd: u32, ...args: Arg[]): i32;
  /** This says whether a command's packet fits a slot. */
  fits(cmd: u32, ...args: Arg[]): boolean;
  /** Asks for a turn. */
  wake(): void;
  /** Tells the policy that an input came now. */
  input(): void;
  /** Lets turns run, while the program is shown. */
  resume(): void;
  /** Stops turns; a call still rings. */
  pause(): void;
  /** Says whether the program is shown. Each change is told to the core
   * (SYS_RESUME, SYS_SUSPEND). */
  visible(visible: boolean): void;
  /** `bind` lends a canvas to the GPU under the core's handle, and `unbind`
   * lets it go. Across a port a canvas is lent once, since it is
   * transferred for good. */
  bind(handle: u32, canvas: Canvas): void;
  unbind(handle: u32): void;
}

export interface Runtime extends Link {
  core: Core;
  head: Head;
  doorbell: Doorbell;
  /** Tells the scheduler of what was submitted on `doorbell` itself. */
  kick(): void;
}

/** This is the memory of the core last mounted here. */
export let mem: ArrayBuffer;

/** Mounts the runtime on `core`. The front is laid as `m` places it, unless
 * it was laid already, and the GPU is granted where `m` asks for one. */
export function mount<G extends Grant>(
  core: Core,
  m: Front & Pick<Machine, "head_at" | "gpu">,
  cfg: Config<G>,
  ui: Ui,
): Runtime {
  if (!laid(core.mem, m.head_at)) lay(core.mem, m.head_at, m);
  mem = core.mem;
  const head = new Head(core.mem, m.head_at);
  const gpu = m.gpu ? cfg.gpu : undefined;
  const policy = cfg.policy;
  const span = cfg.mem_of ?? (() => null);
  const named = (cmd: u32, f: BranchCursor | null) => {
    const s = span(cmd, f);
    return s && new Uint8Array(core.mem, s[0], s[1]);
  };
  const thread = core.thread;

  const doorbell = new Doorbell(
    core.mem,
    head.sq_at,
    head.cq_at,
    (deadline_ms) => {
      const wake = core.run(deadline_ms);
      gpu?.after_turn();
      return wake;
    },
  );
  const scheduler =
    thread ??
    new Scheduler(doorbell, policy, (grant, delta_s) => {
      cfg.submit(grant, delta_s);
      doorbell.send(Command.SYS_TURN, encode_args([Date.now()]));
    });
  if (thread) {
    // A read of the completion ring gives back room a task may wait for.
    doorbell.on_wake = (wake_ms) => {
      if (wake_ms === 0) thread.wake();
    };
    thread.listen((stopped) => doorbell.turns(stopped));
    thread.pause();
  }

  const kick = <T>(v: T): T => (scheduler.wake(), v);
  const send = (cmd: u32, ...args: Arg[]) =>
    kick(doorbell.send(cmd, encode_args(args)));
  function call(cmd: u32, ...args: Arg[]): i32 {
    return kick(
      doorbell.call(cmd, encode_args(args), (res) => {
        if (res < 0)
          console.warn(`[xpute] 0x${cmd.toString(16)}: ${strerror(res)}`);
        return res;
      }),
    );
  }

  const own = new Map<u32, OnImport>([
    [
      Major.SYS,
      sys_imports({
        [Command.SYS_ABORT]: (e) => log_abort(abort_of(e)),
      }),
    ],
  ]);
  if (cfg.fs) {
    const files = new FsServer(cfg.fs, core.mem, (tag, cmd, res, encode) =>
      kick(doorbell.complete(tag, cmd, res, encode)),
    );
    own.set(fs.Major.FS, fs.fs_imports(files));
  }
  const ui_cmds = new Set(ui.cmds);
  doorbell.listen((tag, cmd, f, pkt) => {
    const major = frame.major_of(cmd);
    const on = major < frame.PROGRAM_MAJOR ? own.get(major) : cfg.program;
    const res = on
      ? on(tag, cmd, f, pkt)
      : major === fs.Major.FS
        ? Errno.ENODEV
        : Errno.ENOSYS;
    if (ui_cmds.has(cmd)) ui.on(tag, cmd, f, named(cmd, f), pkt);
    return res;
  });
  doorbell.onterminated = () => {
    scheduler.pause();
    ui.stop();
  };
  core.answer_with((at) => doorbell.answer(at));

  // Turns run while resumed and shown.
  let resumed = false;
  let shown = true;

  if (gpu) {
    gpu.watch(ui.gpu);
    gpu.grant({
      mem: core.mem,
      at: head.gpu_at,
      ask: m.gpu!,
      ready: () => send(GpuCommand.GPU_READY),
      wake: () => scheduler.wake(),
      shown: () => shown,
    });
  }

  function steer(): void {
    if (resumed && shown) scheduler.resume();
    else scheduler.pause();
  }
  function canvas(): Required<Pick<Gpu, "bind" | "unbind">> {
    if (!gpu?.bind || !gpu.unbind) throw new InvariantError(Errno.ENOTSUP);
    return gpu as Required<Gpu>;
  }

  // The core hears a runtime attach before anything else.
  doorbell.send(Command.SYS_ATTACH);
  return {
    core,
    head,
    doorbell,
    send,
    request: (cmd, ...args) => kick(doorbell.request(cmd, encode_args(args))),
    call,
    fits: (_cmd, ...args) => {
      const pkt = encode_args(args)(doorbell);
      return pkt === null || doorbell.fits(pkt);
    },
    kick: () => scheduler.wake(),
    wake: () => scheduler.wake(),
    input: () => (thread ? thread.input() : policy.input?.(performance.now())),
    resume: () => ((resumed = true), steer()),
    pause: () => ((resumed = false), steer()),
    visible(visible) {
      if (visible === shown) return;
      shown = visible;
      steer();
      // SYS_SUSPEND and SYS_RESUME go as a call, rung at once, because hiding
      // is the core's last sure moment to save what it holds.
      call(visible ? Command.SYS_RESUME : Command.SYS_SUSPEND);
    },
    bind: (handle, c) => canvas().bind(handle, c),
    unbind: (handle) => canvas().unbind(handle),
  };
}

/** This is a core's machine, as its generated `MACHINE` gives it: the memory
 * sizes it allows, the front and where its header is, and the devices it asks
 * for. */
export interface Machine extends Front {
  mem_bytes: readonly u32[];
  region_bytes: u32;
  head_at: u32;
  fs: boolean;
  gpu: GpuAsk | null;
}

/** This says where a page's core comes from: its module, fetched from a URL or
 * given as bytes, and the size of its memory in bytes, which its machine must
 * allow. */
export interface ModuleSource {
  module: URL | BufferSource;
  bytes: u32;
  init?: RequestInit;
}

/** This is an app's core, declared once: its machine, and a function that
 * makes its config where the core runs. */
export interface Manifest<G extends Grant = Grant> {
  machine: Machine;
  core(): Config<G> | Promise<Config<G>>;
}

/** Opens the core of `m` on this build's platform and mounts the runtime on it;
 * what the core tells a UI goes to `ui`. A page's core takes `now` as its
 * clock, which counts as the UI's `performance.now()` does; a native core keeps
 * xpute-runtime's own (platform.native.ts). It refuses with EINVAL a config
 * without `fs` for a machine that asks for files, and a config with `fs` for a
 * machine that does not. A GPU the machine does not ask for is not granted. */
export async function boot<G extends Grant>(
  m: Manifest<G>,
  ui: Ui,
  now: () => f64 = () => performance.now(),
): Promise<Runtime> {
  const cfg = await m.core();
  if (m.machine.fs !== (cfg.fs !== undefined)) {
    throw new InvariantError(Errno.EINVAL);
  }
  return mount(await open(m.machine, cfg.core, now), m.machine, cfg, ui);
}
