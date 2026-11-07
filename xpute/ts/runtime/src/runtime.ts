// @xpute/runtime/runtime.ts

/**
 * A runtime mounted on a core its platform opened.
 *
 * - The front is laid before the first turn (ipc/head.ts).
 * - Each turn is rung by the doorbell, which reads what the core left as the
 *   turn ends, a trapped turn's too.
 * - What the core posts is answered by its major: xpute's own below
 *   PROGRAM_MAJOR (the stop report, the file device), the program's from
 *   there; each the UI takes goes to it too.
 * - The scheduler runs turns as the policy says (sched/policy.ts).
 */

import type { f64, i32, u32 } from "@xpute/kit/abi/word.ts";
import type { U8Array } from "@xpute/kit/abi/array.ts";
import * as Errno from "@xpute/kit/status/errno.spec.ts";
import { strerror } from "@xpute/kit/status/errno.spec.ts";
import { InvariantError } from "@xpute/kit/status/error.ts";
import type { BranchCursor } from "@xpute/kit/wire/xtp/mod.ts";
import {
  Command,
  encode_sys_turn,
  Major,
  read_sys_abort,
  sys_imports,
  type SysAbort,
} from "@xpute/runtime/abi/sys.spec.ts";
import { Command as GpuCommand } from "@xpute/runtime/abi/gpu.spec.ts";
import * as fs from "@xpute/runtime/abi/fs.spec.ts";
import { type Front, Head, laid, lay } from "@xpute/runtime/ipc/head.ts";
import * as frame from "@xpute/runtime/ipc/frame.ts";
import {
  Doorbell,
  type Encode,
  type OnImport,
} from "@xpute/runtime/ipc/doorbell.ts";
import { type Fs, FsServer } from "@xpute/runtime/ipc/fs.ts";
import type { Grant, SchedulingPolicy } from "@xpute/runtime/sched/policy.ts";
import { Scheduler } from "@xpute/runtime/sched/scheduler.ts";
import { open } from "./platform";

/** The core as its platform opened it: its memory, which never moves or
 * grows; `run`, a turn, which returns when it wants the next, in ms; where
 * its calls are answered; and, where it runs on its own thread, that thread.
 * xpute names the entry, as a C runtime names `main`. */
export interface Core {
  mem: ArrayBuffer;
  run(deadline_ms: f64): f64;
  /** `answer` answers each call from here on; ENOSYS until it is set. */
  answer_with(answer: (at: u32) => i32): void;
  /** A native core's thread, which runs while it has work and sleeps until
   * kicked: `kick` wakes it, a turn given at least `deadline_ms`; `heard`
   * is told when it wants the next turn (NaN once it stopped) and the
   * longest turn it ran; `pause` stops its turns, `resume` lets kicks run
   * them. None for a page's core, which runs only inside `run`. */
  thread?: {
    kick(deadline_ms: f64): void;
    pause(): void;
    resume(): void;
    listen(heard: (wake_ms: f64, turn_ms: f64) => void): void;
  };
}

/** What a core asks of its GPU (its machine's `gpu`), by WebGPU's names. */
export interface GpuAsk {
  power: string;
  features: { required: readonly string[]; optional: readonly string[] };
}

/** A platform's GPU: it opens a device as the core asks, runs the core's
 * stream as each turn returns, says when a device answers and whether there
 * is one, and draws the canvases it is lent. What `after_turn` throws ends
 * the core. A native GPU binds its views itself and takes no canvas. */
export interface Gpu {
  grant(to: GpuGrant): void;
  after_turn(): void;
  watch(report: (state: GpuState) => void): void;
  bind?(handle: u32, canvas: Canvas): void;
  unbind?(handle: u32): void;
}

export type GpuState =
  "present" | "waiting" | "missing" | "lost" | "unsupported" | "insecure";

/** A page's canvas, or one transferred to the worker its runtime runs in. */
export type Canvas = HTMLCanvasElement | OffscreenCanvas;

/** The core's memory, where its stream begins, what it asks; `ready` tells
 * the core a device answered (GPU_READY), `wake` asks a turn, `shown` says
 * whether the UI shows the core (`Link.visible`). */
export interface GpuGrant {
  mem: ArrayBuffer;
  at: u32;
  ask: GpuAsk;
  ready(): void;
  wake(): void;
  shown(): boolean;
}

/** The core's report as it aborts: the errno it broke, where, and two
 * numbers it gave. */
export interface Abort {
  errnum: number;
  file: string;
  line: u32;
  column: u32;
  a: bigint;
  b: bigint;
}

/** The report a SYS_ABORT carries. */
export function read_abort(f: BranchCursor): Abort {
  return abort_of(read_sys_abort(f));
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

/** What the core runs with, all the app decides: its module where a page
 * loads one; what backs its files and the GPU it is granted, where its
 * machine asks; what answers its commands; the policy and what goes with
 * each turn; and where an import names its memory, as its protocol's
 * generated `<name>_mem` says. */
export interface Config<G extends Grant = Grant> {
  core?: ModuleSource;
  fs?: Fs;
  gpu?: Gpu;
  program?: OnImport;
  policy: SchedulingPolicy<G>;
  submit(grant: G, delta_s: f64): void;
  mem_of?(cmd: u32, f: BranchCursor | null): [u32, u32] | null;
}

/** What a UI hears of its core: the imports it takes (`cmds`, SYS_ABORT
 * among them where it shows the stop), with `named` the memory one names
 * and `pkt` its packet; the GPU's state; the stop. */
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

/** A UI that hears nothing. */
export const QUIET: Ui = { cmds: [], on() {}, gpu() {}, stop() {} };

/** What a protocol's generated stubs send through. */
export interface Sender {
  /** Sent with the next turn. */
  send(cmd: u32, encode?: Encode): void;
  /** Sent with ACKREQ; the core's reply comes with a later turn. */
  request(cmd: u32, encode?: Encode | null): Promise<i32>;
  /** Rung at once until the core answers, turns paused or not. */
  call(cmd: u32, encode?: Encode | null): i32;
  call<T>(
    cmd: u32,
    encode: Encode | null,
    read: (result: i32, reply: BranchCursor | null) => T,
  ): T;
}

/** The UI's link to its runtime, beside it or across a port. */
export interface Link extends Sender {
  /** Whether a command's packet fits a slot. */
  fits(encode: Encode): boolean;
  /** A turn asked for. */
  wake(): void;
  /** An input now, for the policy. */
  input(): void;
  /** Turns run, while the program is shown. */
  resume(): void;
  /** No turn runs; a call still rings. */
  pause(): void;
  /** Shown or hidden; each change told the core (SYS_RESUME, SYS_SUSPEND). */
  visible(visible: boolean): void;
  /** A canvas lent to the GPU under the core's handle, and let go. Across a
   * port a canvas is lent once: it crosses for good. */
  bind(handle: u32, canvas: Canvas): void;
  unbind(handle: u32): void;
}

export interface Runtime extends Link {
  core: Core;
  head: Head;
  doorbell: Doorbell;
}

/** The memory of the core last mounted here. */
export let mem: ArrayBuffer;

/** The runtime on `core`, its front laid as `m` places it unless laid, the
 * GPU granted where `m` asks for one. */
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
    thread ? (deadline_ms) => thread.kick(deadline_ms) : null,
  );
  const scheduler = new Scheduler(doorbell, policy, (grant, delta_s) => {
    cfg.submit(grant, delta_s);
    doorbell.send(Command.SYS_TURN, (room) =>
      encode_sys_turn(room, Date.now()),
    );
  });
  thread?.listen((wake_ms, turn_ms) => doorbell.turns(wake_ms, turn_ms));
  thread?.pause();

  const kick = <T>(v: T): T => (scheduler.kick(), v);
  const send = (cmd: u32, encode?: Encode) => kick(doorbell.send(cmd, encode));
  function call<T>(
    cmd: u32,
    encode: Encode | null = null,
    read?: (result: i32, reply: BranchCursor | null) => T,
  ): T | i32 {
    return kick(
      doorbell.call(cmd, encode, (res, reply) => {
        if (read) return read(res, reply);
        if (res < 0)
          console.warn(`[xpute] 0x${cmd.toString(16)}: ${strerror(res)}`);
        return res;
      }),
    );
  }

  const own = new Map<u32, OnImport>([
    [Major.SYS, sys_imports({ sys_abort: (e) => log_abort(abort_of(e)) })],
  ]);
  if (cfg.fs) {
    const files = new FsServer(cfg.fs, core.mem, (tag, cmd, res, encode) =>
      kick(doorbell.complete(tag, cmd, res, encode)),
    );
    own.set(fs.Major.FS, fs.fs_imports(files));
  }
  const taken = new Set(ui.cmds);
  doorbell.listen((tag, cmd, f, pkt) => {
    const major = frame.major_of(cmd);
    const on = major < frame.PROGRAM_MAJOR ? own.get(major) : cfg.program;
    const res = on
      ? on(tag, cmd, f, pkt)
      : major === fs.Major.FS
        ? Errno.ENODEV
        : Errno.ENOSYS;
    if (taken.has(cmd)) ui.on(tag, cmd, f, named(cmd, f), pkt);
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
    if (resumed && shown) {
      thread?.resume();
      scheduler.resume();
    } else {
      scheduler.pause();
      thread?.pause();
    }
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
    request: (cmd, encode) => kick(doorbell.request(cmd, encode ?? null)),
    call,
    fits: (encode) => {
      const pkt = encode(doorbell);
      return pkt === null || doorbell.fits(pkt);
    },
    wake: () => scheduler.wake(),
    input: () => policy.input?.(performance.now()),
    resume: () => ((resumed = true), steer()),
    pause: () => ((resumed = false), steer()),
    visible(visible) {
      if (visible === shown) return;
      shown = visible;
      steer();
      // At once: hiding is the core's last sure moment to keep what it holds.
      call(visible ? Command.SYS_RESUME : Command.SYS_SUSPEND);
    },
    bind: (handle, c) => canvas().bind(handle, c),
    unbind: (handle) => canvas().unbind(handle),
  };
}

/** A core's machine as its generated `MACHINE` gives it: the memory sizes it
 * allows, where the front's header is and the front, and the devices it asks
 * for. */
export interface Machine extends Front {
  mem_bytes: readonly u32[];
  region_bytes: u32;
  head_at: u32;
  fs: boolean;
  gpu: GpuAsk | null;
}

/** A page's core: its module, fetched or given as bytes, and the bytes of its
 * memory, a size its machine allows. */
export interface ModuleSource {
  module: URL | BufferSource;
  bytes: u32;
  init?: RequestInit;
}

/** An app's core, declared once: its machine, and its config, made where the
 * core runs. */
export interface Manifest<G extends Grant = Grant> {
  machine: Machine;
  core(): Config<G> | Promise<Config<G>>;
}

/** The core of `m` opened by this build's platform and the runtime mounted
 * on it, what it tells a UI told `ui`. A machine that asks for files is
 * refused a config without `fs`, and one that does not, one with it; a GPU
 * the machine does not ask for is not granted. */
export async function boot<G extends Grant>(
  m: Manifest<G>,
  ui: Ui,
): Promise<Runtime> {
  const cfg = await m.core();
  if (m.machine.fs !== (cfg.fs !== undefined)) {
    throw new InvariantError(Errno.EINVAL);
  }
  return mount(await open(m.machine, cfg.core), m.machine, cfg, ui);
}
