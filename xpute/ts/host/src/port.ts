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
  PacketRoom,
  PacketWriter,
} from "@xpute/kit/wire/xtp/mod.ts";

import { type Host, mount } from "./host.ts";
import { op } from "./ipc/sys.spec.ts";
import * as program from "./program.spec.ts";
import type { Fs } from "./ipc/sys.ts";
import { open_module } from "./module.ts";
import { FixedQuota } from "./sched/fixed_quota.ts";
import type { Grant, SchedulingPolicy } from "./sched/policy.ts";

/** A value a command carries, each lowered as XTP lowers it. */
export type Arg = NodeValue<NodeView>;

/** What the guest raised, valid only inside the call. */
export type Listener = (pkt: BranchCursor | null) => void;

/** A command as its table generates it (xpute-tool `spec/cmd`): its number,
 * and its packet's writer, which takes its fields by place, so a bundle keeps
 * no field's name; none for a command with no packet. */
export interface Command<A extends unknown[]> {
  readonly number: u32;
  readonly write?: (to: PacketRoom, ...fields: A) => PacketWriter;
}

/** An event as its table generates it: its number, and its reader, which
 * gives its fields by place. */
export interface Event<A extends unknown[]> {
  readonly number: u32;
  readonly read: (b: BranchCursor) => A;
}

export interface Port<G extends Grant = Grant> {
  /** The port's number, which the guest sees as the sender. */
  readonly number: u32;
  /** Applied on the next turn; its result, if it fails, is logged. */
  send<A extends unknown[]>(cmd: Command<A>, ...fields: A): void;
  send(cmd: u32, ...args: Arg[]): void;
  /** Applied now, the guest rung with no quota until it answers. */
  call<A extends unknown[]>(cmd: Command<A>, ...fields: A): i32;
  call(cmd: u32, ...args: Arg[]): i32;
  /** What the guest raises for this port, until the returned call. */
  on<A extends unknown[]>(
    event: Event<A>,
    heard: (...fields: A) => void,
  ): () => void;
  on(cmd: u32, heard: Listener): () => void;
  /** The mount underneath. */
  readonly host: Host<G>;
}

function answered(result: i32): i32 {
  if (result < 0) throw new XputeError(result);
  return result;
}

/** The verbs over a mounted guest, sent as `number`; one port a number. */
export function port<G extends Grant>(host: Host<G>, number: u32): Port<G> {
  const hearing = new Map<u32, Listener>();
  host.doorbell.listen(number, (_tag, cmd, pkt) => hearing.get(cmd)?.(pkt));
  const room: PacketRoom = host.doorbell;
  // A table's command writes its own packet; a bare number's are lowered.
  const packet = (cmd: u32 | Command<unknown[]>, args: unknown[]) =>
    typeof cmd === "number"
      ? host.doorbell.lower(args as Arg[])
      : (cmd.write?.(room, ...args) ?? null);
  const number_of = (cmd: u32 | { number: u32 }) =>
    typeof cmd === "number" ? cmd : cmd.number;
  return {
    number,
    host,
    send: (cmd: u32 | Command<unknown[]>, ...args: unknown[]) =>
      host.send(number, number_of(cmd), packet(cmd, args) ?? undefined),
    call: (cmd: u32 | Command<unknown[]>, ...args: unknown[]) =>
      host.doorbell.call(number, number_of(cmd), packet(cmd, args), answered),
    on(
      cmd: u32 | Event<unknown[]>,
      heard: Listener | ((...fields: unknown[]) => void),
    ) {
      const key = number_of(cmd);
      const fields = heard as (...fields: unknown[]) => void;
      const take: Listener =
        typeof cmd === "number"
          ? (heard as Listener)
          : (pkt) => fields(...(pkt ? cmd.read(pkt) : []));
      hearing.set(key, take);
      return () => {
        if (hearing.get(key) === take) hearing.delete(key);
      };
    },
  };
}

/** The port a program speaks as unless it says otherwise: any but the host's, 0. */
export const PROGRAM_PORT: u32 = program.PORT;

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
 * A guest built with `xpute_guest::program!` or `XPUTE_PROGRAM`, opened and
 * mounted: its module instantiated over a memory of its own, its `boot` asked
 * where its toc lies, its turns begun (stopped by `host.strobe.stop`), and the
 * port the program speaks as.
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
  host.strobe.start();
  return port(host, opts.port ?? PROGRAM_PORT);
}
