// @xpute/example/cmd.spec.ts
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
import type { u32 } from "@xpute/kit/abi/word.ts";
import type { Raised, Sent } from "@xpute/host/port.ts";
import * as Command from "./cmd/command.spec.ts";

/** One of Command's, as its enum was a type. */
type Command = (typeof Command)[keyof typeof Command];
export { Command };

/** Command.SUM's packet, written in place into `to`. */
export function encode_sum(to: PacketRoom, a: u32, b: u32): PacketWriter {
  return to.writer(2).u32(a).u32(b);
}

/** Command.SUM, as a port sends it or hears it. */
export const SUM: Sent<[a: u32, b: u32]> = {
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
export const SUMMED: Raised<[total: u32]> = {
  number: Command.SUMMED,
  read: read_summed_tuple,
};
