// ts/example/cmd.spec.ts
//
// GENERATED from spec/example/cmd.json

/**
 * The example's own commands (rust/example, ts/example/sum.test.ts): a program's
 * table, from which its guest's readers and its host's writers are generated.
 */

import type {
  BranchCursor,
  PacketRoom,
  PacketWriter,
} from "@xpute/kit/wire/xtp/mod.ts";
import type { i32, u32 } from "@xpute/kit/abi/word.ts";
import type * as port from "@xpute/host/port.ts";
import * as Hostcall from "./cmd/hostcall.spec.ts";
import * as Root from "./cmd/root.spec.ts";
import * as Command from "./cmd/command.spec.ts";

/** One of Hostcall's, as its enum was a type. */
type Hostcall = (typeof Hostcall)[keyof typeof Hostcall];
export { Hostcall };

/** One of Root's, as its enum was a type. */
type Root = (typeof Root)[keyof typeof Root];
export { Root };

/** One of Command's, as its enum was a type. */
type Command = (typeof Command)[keyof typeof Command];
export { Command };

/** Command.SUM's packet, written in place into `to`. */
export function encode_sum(to: PacketRoom, a: u32, b: u32): PacketWriter {
  return to.writer(2).u32(a).u32(b);
}

/** Command.SUM, as a port sends it or hears it. */
export const SUM: port.Command<[a: u32, b: u32]> = {
  number: Command.SUM,
  write: encode_sum,
};

/** Command.SUMMED's event. */
export interface SummedEvent {
  total: u32;
}

export function read_summed(b: BranchCursor): SummedEvent {
  return { total: b.at(0).get<u32>() };
}

export function read_summed_tuple(b: BranchCursor): [total: u32] {
  return [b.at(0).get<u32>()];
}

/** Command.SUMMED, as a port sends it or hears it. */
export const SUMMED: port.Event<[total: u32]> = {
  number: Command.SUMMED,
  read: read_summed_tuple,
};

/** Command.COUNT's packet, written in place into `to`. */
export function encode_count(to: PacketRoom, path: string): PacketWriter {
  return to.writer(1).str(path);
}

/** Command.COUNT, as a port sends it or hears it. */
export const COUNT: port.Command<[path: string]> = {
  number: Command.COUNT,
  write: encode_count,
};

/** Command.COUNTED's event. */
export interface CountedEvent {
  res: i32;
}

export function read_counted(b: BranchCursor): CountedEvent {
  return { res: b.at(0).get<i32>() };
}

export function read_counted_tuple(b: BranchCursor): [res: i32] {
  return [b.at(0).get<i32>()];
}

/** Command.COUNTED, as a port sends it or hears it. */
export const COUNTED: port.Event<[res: i32]> = {
  number: Command.COUNTED,
  read: read_counted_tuple,
};
