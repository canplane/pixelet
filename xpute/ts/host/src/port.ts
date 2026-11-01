// @xpute/host/port.ts

/**
 * A guest as a program talks to it: one port, and a verb for each way a
 * message crosses. On the rings, taken later: `send`, a command the next turn
 * applies, and `on`, what the guest raises, heard after the turn that raised
 * it. Directly, answered before it returns: `call`. What lies under them, the
 * toc, the rings and their slots, the frames' tags and the turns, is the
 * mount's (host.ts), which a program that lays its guest out itself uses
 * as it is.
 *
 * A result below 0 is an errno and is thrown as one (`XputeError`); a ring
 * with no room is waited out, as a full pipe is.
 */

import type { f64, i32, u32 } from "@xpute/kit/abi/word.ts";
import type { U32Array } from "@xpute/kit/abi/array.ts";
import * as Errno from "@xpute/kit/status/errno.spec.ts";
import { XputeError } from "@xpute/kit/status/error.ts";
import type {
  BranchCursor,
  NodeValue,
  NodeView,
} from "@xpute/kit/wire/xtp/mod.ts";

import { type Host, mount } from "./host.ts";
import { op } from "./ipc/sys.spec.ts";
import type { Fs } from "./ipc/sys.ts";
import { open_module } from "./module.ts";
import { FixedQuota } from "./sched/fixed_quota.ts";
import type { Grant, SchedulingPolicy } from "./sched/policy.ts";

/** A value a command carries, each lowered as XTP lowers it. */
export type Arg = NodeValue<NodeView>;

/** What the guest raised, valid only inside the call. */
export type Heard = (pkt: BranchCursor | null) => void;

export interface Port<G extends Grant = Grant> {
  /** The port's number, which the guest sees as the sender. */
  readonly number: u32;
  /** Applied on the next turn; its result, if it fails, is logged. */
  send(cmd: u32, ...args: Arg[]): void;
  /** Applied now, the guest rung with no quota until it answers. */
  call(cmd: u32, ...args: Arg[]): i32;
  /** What the guest raises as `cmd` for this port, until the returned call. */
  on(cmd: u32, heard: Heard): () => void;
  /** The mount underneath. */
  readonly host: Host<G>;
}

function answered(result: i32): i32 {
  if (result < 0) throw new XputeError(result);
  return result;
}

/** The verbs over a mounted guest, sent as `number`; one port a number. */
export function port<G extends Grant>(host: Host<G>, number: u32): Port<G> {
  const hearing = new Map<u32, Heard>();
  host.doorbell.listen(number, (_tag, cmd, pkt) => hearing.get(cmd)?.(pkt));
  return {
    number,
    host,
    send: (cmd, ...args) =>
      host.send(number, cmd, host.doorbell.lowered(args) ?? undefined),
    call: (cmd, ...args) =>
      host.doorbell.call(number, cmd, host.doorbell.lowered(args), answered),
    on(cmd, heard) {
      hearing.set(cmd, heard);
      return () => {
        if (hearing.get(cmd) === heard) hearing.delete(cmd);
      };
    },
  };
}

/** The port a program speaks as unless it says otherwise: any but the host's, 0. */
export const PROGRAM_PORT: u32 = 1;

export interface Options<G extends Grant = Grant> {
  /** Its hostcalls of the program's own, by number (`PROGRAM` set or not):
   * each handed its arguments, its answer what it returns. */
  answer?: Record<u32, (args: U32Array) => i32>;
  /** What its files are opened against. */
  fs?: Fs;
  /** When turns run and how much each gets: a fixed 4 ms by default, for a
   * host with no display. */
  policy?: SchedulingPolicy<G>;
  /** Fills the rings before each turn the policy grants. */
  submit?(grant: G, delta_s: f64): void;
  /** The memory it runs in, never grown: 16 MiB by default. */
  memory?: WebAssembly.Memory;
  /** The port the program speaks as. */
  port?: u32;
}

/**
 * A guest built with `xpute_guest::program!`, opened and mounted: its module
 * instantiated over a memory of its own, its `boot` asked where its toc lies,
 * and the port the program speaks as.
 */
export function open(module: WebAssembly.Module, opts?: Options): Port;
export function open<G extends Grant>(
  module: WebAssembly.Module,
  opts: Options<G> & { policy: SchedulingPolicy<G> },
): Port<G>;
export function open<G extends Grant>(
  module: WebAssembly.Module,
  opts: Options<G> = {},
): Port<G> {
  const memory =
    opts.memory ?? new WebAssembly.Memory({ initial: 256, maximum: 256 });
  const guest = open_module(module, memory);
  const boot = guest.instance.exports.boot;
  if (typeof boot !== "function") throw new XputeError(Errno.ENOEXEC);
  const answer = opts.answer ?? {};
  const host = mount(guest, {
    toc_at: (boot as () => number)() >>> 0,
    sys: {
      fs: opts.fs,
      program: (code, w, a) =>
        (answer[code] ?? answer[(code & ~op.PROGRAM) >>> 0])?.(
          w.subarray(a) as U32Array,
        ) ?? Errno.ENOSYS,
    },
    // With no policy given, G is Grant (the overloads), which FixedQuota's is.
    policy:
      opts.policy ?? (new FixedQuota(4) as unknown as SchedulingPolicy<G>),
    submit: opts.submit ?? (() => {}),
  });
  return port(host, opts.port ?? PROGRAM_PORT);
}
