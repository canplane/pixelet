// @xpute/host/host.ts

/**
 * A host mounted on a guest its platform opened: the front it lays before
 * the first turn (ipc/head.ts), a doorbell whose every turn reads what the
 * guest left as it ends (a trapped turn's too, so its report is read before
 * the doorbell hears it stop), what answers the guest's commands by their
 * major — xpute's own below PROGRAM_MAJOR (abi/cmd.json: the guest's report
 * as it aborts; the file device where the program backs one, ipc/fs.ts), the
 * program's from there — and the strobe, which runs turns as the scheduling
 * policy the program boots it with says (sched/policy.ts).
 */

import type { f64, i32, u32 } from "@xpute/kit/abi/word.ts";
import * as Errno from "@xpute/kit/status/errno.spec.ts";
import { strerror } from "@xpute/kit/status/errno.spec.ts";
import type { BranchCursor } from "@xpute/kit/wire/xtp/mod.ts";
import {
  Command,
  Major,
  host_imports,
  type SysAbortEvent,
} from "@xpute/host/abi/cmd.spec.ts";
import * as fs from "@xpute/host/abi/fs.spec.ts";
import { type Front, Head, laid, lay } from "@xpute/host/ipc/head.ts";
import * as frame from "@xpute/host/ipc/frame.ts";
import {
  Doorbell,
  type Encode,
  type OnEvent,
} from "@xpute/host/ipc/doorbell.ts";
import { type Fs, FsServer } from "@xpute/host/ipc/fs.ts";
import type { Grant, SchedulingPolicy } from "@xpute/host/sched/policy.ts";
import { Strobe } from "@xpute/host/sched/strobe.ts";
import { open } from "./platform";

/** The guest as its platform opened it: its memory, which never moves or
 * grows; its entry, `run`, a turn that returns when it wants the next, in
 * ms; where its hostcalls are answered; and, where it runs on a thread of
 * its own, that thread.
 * The floor names the entry, as a C runtime names `main`. */
export interface Guest {
  mem: ArrayBuffer;
  run(quota_ms: f64): f64;
  /** `answer` answers each call from here on: a frame laid at an offset of
   * `mem`, answered before the call returns. ENOSYS until it is set. */
  answer_with(answer: (at: u32) => i32): void;
  /** The guest's own thread, which runs while it has work and sleeps until
   * it is kicked or the time it asked for comes (a native app's): `kick`
   * wakes it after a submission, its next turn given at least `quota_ms`,
   * none of its own where negative, and returns at once; `listen`'s `ack`
   * is called on this thread once it has posted or asked since, with when it
   * wants the next turn (0 also where its wake time began one, as the thread
   * keeps that time; NaN once it has stopped) and the longest turn it ran.
   * `pause` stops every turn but one `run` rings, and `resume` lets
   * kicks and the wake time run turns again. `run` is then a turn waited
   * for. None where the guest runs only inside `run` (a page's). */
  thread?: {
    kick(quota_ms: f64): void;
    pause(): void;
    resume(): void;
    listen(ack: (wake_ms: f64, turn_ms: f64) => void): void;
  };
}

/** What a guest asks of the GPU it is granted (its machine's `gpu`): a power
 * preference, and features the device must have and those it takes where the
 * adapter has them, by WebGPU's names. */
export interface GpuAsk {
  power: string;
  features: { required: readonly string[]; optional: readonly string[] };
}

/** A platform's GPU, granted to a guest: it opens a device as the guest asks,
 * runs the guest's stream on it as each turn returns (a native one, as the
 * guest publishes it), and says when a device has answered. What
 * `after_turn` throws ends the guest, as a trap does. */
export interface Gpu {
  grant(to: GpuGrant): void;
  after_turn(): void;
}

/** What a GPU is granted to: the guest's memory, which never moves; where its
 * stream begins; what it asks; `ready`, which tells the guest a device
 * answered (GPU_READY); and `wake`, a turn asked for, as a
 * completion the guest did not hear of came. */
export interface GpuGrant {
  mem: ArrayBuffer;
  at: u32;
  ask: GpuAsk;
  ready(): void;
  wake(): void;
}

/** The guest's report as it aborts: the errno of the invariant it broke, where,
 * and two numbers it gave. */
export interface Abort {
  errnum: number;
  file: string;
  line: u32;
  column: u32;
  a: bigint;
  b: bigint;
}

function abort_of(e: SysAbortEvent): Abort {
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

/** What the program mounts its guest with: all it decides, and nothing of
 * the order the host is put together in. */
export interface Config<G extends Grant = Grant> {
  /** Where the front's header is. */
  head_at: u32;
  /** The front the host lays there before the first turn, unless one is
   * laid: a guest that outlived its host's reload keeps its rings as they
   * are. Unset for a guest that laid its own. */
  lay?: Front;
  /** What backs the file device (Major FS), where it is given. */
  fs?: Fs;
  /** The guest's report as it aborts (SYS_ABORT); logged where
   * none takes it. */
  onabort?(report: Abort): void;
  /** The GPU granted and what the guest asks of it (set by `boot` from the
   * machine); its stream is where the header says. */
  gpu?: { by: Gpu; ask: GpuAsk };
  /** What the program hears the guest send or call it with, its commands
   * those of PROGRAM_MAJOR and up; a call it does not take is ENOSYS. */
  program?: OnEvent;
  /** When turns run and how much each gets, the program's policy. */
  policy: SchedulingPolicy<G>;
  /** Fills the rings before each turn the strobe grants is rung. */
  submit(grant: G, delta_s: f64): void;
  /** Told once the guest has stopped. */
  onterminated?(): void;
}

export interface Host {
  guest: Guest;
  head: Head;
  doorbell: Doorbell;
  /** Sent with the next turn, its packet written by `encode`; while the
   * completion ring is read, once it is. */
  send(cmd: u32, encode?: Encode): void;
  /** Rung at once with no quota until the guest answers, so it is heard
   * while turns are paused too; then a turn is asked for. `read` reads the
   * answer; without it, the result is returned and a failure is logged, as
   * an answer no one reads is. */
  call(cmd: u32, encode?: Encode | null): i32;
  call<T>(
    cmd: u32,
    encode: Encode | null,
    read: (result: i32, reply: BranchCursor | null) => T,
  ): T;
  /** A turn asked for, as something the guest did not hear of changed. */
  wake(): void;
  /** Turns run from now on, as the policy gives them, while the platform
   * shows the program; nothing where they already run or the guest stopped. */
  resume(): void;
  /** No turn runs until `resume`; a `call` still rings at once. */
  pause(): void;
  /** Whether the platform shows the program: a hidden one runs no turn, as a
   * hidden page gets no frame. Each change is told the guest at once
   * (SYS_SUSPEND, SYS_RESUME). */
  visible(visible: boolean): void;
}

export function mount<G extends Grant>(guest: Guest, cfg: Config<G>): Host {
  if (cfg.lay && !laid(guest.mem, cfg.head_at)) {
    lay(guest.mem, cfg.head_at, cfg.lay);
  }
  const head = new Head(guest.mem, cfg.head_at);
  const gpu = cfg.gpu?.by;
  const doorbell = new Doorbell(
    guest.mem,
    head.sq_at,
    head.cq_at,
    (quota_ms) => {
      const wake = guest.run(quota_ms);
      gpu?.after_turn();
      return wake;
    },
    guest.thread ? (quota_ms) => guest.thread?.kick(quota_ms) : null,
  );
  guest.thread?.listen((wake_ms, turn_ms) => doorbell.ack(wake_ms, turn_ms));
  const strobe = new Strobe(doorbell, cfg.policy, cfg.submit);
  let resumed = false;
  let shown = true;
  const steer = () => {
    if (resumed && shown) {
      guest.thread?.resume();
      strobe.resume();
    } else {
      strobe.pause();
      guest.thread?.pause();
    }
  };
  // A guest on its own thread runs no turn of its own before `resume`, as a
  // page's is rung for none.
  guest.thread?.pause();
  const send = (cmd: u32, encode?: Encode) => {
    doorbell.send(cmd, encode);
    strobe.kick();
  };
  const own = new Map<u32, OnEvent>([
    [
      Major.SYS,
      host_imports({
        sys_abort: (e) => (cfg.onabort ?? log_abort)(abort_of(e)),
      }),
    ],
  ]);
  if (cfg.fs) {
    const files = new FsServer(cfg.fs, guest.mem, (tag, cmd, res, encode) => {
      doorbell.complete(tag, cmd, res, encode);
      strobe.kick();
    });
    own.set(fs.Major.FS, fs.fs_imports(files));
  }
  doorbell.listen((tag, cmd, f) => {
    const major = frame.major_of(cmd);
    const listener = major < frame.PROGRAM_MAJOR ? own.get(major) : cfg.program;
    if (listener) return listener(tag, cmd, f);
    return major === fs.Major.FS ? Errno.ENODEV : Errno.ENOSYS;
  });
  guest.answer_with((at) => doorbell.answer(at));
  if (cfg.gpu) {
    cfg.gpu.by.grant({
      mem: guest.mem,
      at: head.gpu_at,
      ask: cfg.gpu.ask,
      ready: () => send(Command.GPU_READY),
      wake: () => strobe.request(),
    });
  }
  doorbell.onterminated = () => {
    strobe.pause();
    cfg.onterminated?.();
  };
  function call<T>(
    cmd: u32,
    encode: Encode | null = null,
    read?: (result: i32, reply: BranchCursor | null) => T,
  ): T | i32 {
    const out = doorbell.call(cmd, encode, (result, reply) => {
      if (read) return read(result, reply);
      if (result < 0) {
        console.warn(
          `[xpute/host] command 0x${cmd.toString(16)} failed: ${strerror(result)}`,
        );
      }
      return result;
    });
    strobe.kick();
    return out;
  }
  // The guest hears a host attach before anything else it is sent.
  doorbell.send(Command.SYS_ATTACH);
  return {
    guest,
    head,
    doorbell,
    send,
    call,
    wake: () => strobe.request(),
    resume: () => {
      resumed = true;
      steer();
    },
    pause: () => {
      resumed = false;
      steer();
    },
    visible: (visible) => {
      if (visible === shown) return;
      shown = visible;
      steer();
      // At once: a hidden page gets no frame, and hiding is the last sure
      // moment the guest has to keep what it holds.
      call(visible ? Command.SYS_RESUME : Command.SYS_SUSPEND);
    },
  };
}

/** What the host reads of a guest's machine (its generated `MACHINE`): the
 * memory sizes it allows and their region, where the front's header is and
 * the front the host lays there, and the GPU it asks for, none where it takes
 * no GPU. What the program's commands carry is the program's. */
export interface Machine extends Front {
  mem_bytes: readonly u32[];
  region_bytes: u32;
  head_at: u32;
  /** Whether it asks for the file device. */
  fs: boolean;
  gpu: GpuAsk | null;
}

/** Where a page finds its guest: its module, fetched from a URL or given as
 * bytes, `init` the fetch's, and the bytes of its memory, one of the sizes
 * its machine allows. A native guest is linked in and takes none. */
export interface ModuleSource {
  module: URL | BufferSource;
  bytes: u32;
  init?: RequestInit;
}

/** The guest opened as `machine` declares, by this build's platform
 * (platform.ts, platform.native.ts), from `guest` on a page; the host
 * mounted on it, its front laid, the file device backed by `fs` and the GPU
 * granted by `gpu` where its machine asks for them. A machine that asks for
 * files is refused a boot without `fs`, and one that does not is refused one
 * with it. */
export async function boot<G extends Grant>(
  machine: Machine,
  cfg: Omit<Config<G>, "head_at" | "lay" | "gpu"> & {
    guest?: ModuleSource;
    gpu?: Gpu;
  },
): Promise<Host> {
  const { guest, gpu, ...rest } = cfg;
  if (machine.fs !== (rest.fs !== undefined)) {
    throw new Error(
      machine.fs
        ? "the machine asks for the file device, and the program backs none"
        : "the program backs a file device the machine does not ask for",
    );
  }
  return mount(await open(machine, guest), {
    ...rest,
    head_at: machine.head_at,
    lay: machine,
    gpu: gpu && machine.gpu ? { by: gpu, ask: machine.gpu } : undefined,
  });
}
