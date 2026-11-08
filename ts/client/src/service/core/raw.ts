// @pxl/client/service/core/raw.ts

/** The core's commands by their numbers, past each major's own stubs. */

import type { u32 } from "@xpute/kit/abi/word.ts";
import type { NodeValue, NodeView } from "@xpute/kit/wire/xtp/mod.ts";
import { encode_args } from "@xpute/runtime";
import { MACHINE } from "@/abi/machine.spec.ts";
import { ui } from "@/service/core/core.ts";

/** The `cmd` of a development console. It sends any command by its number,
 * with its arguments as the fields of its packet, and rings the core. */
export const raw = {
  cmd: (cmd: u32, ...args: NodeValue<NodeView>[]): void =>
    ui.send(cmd, encode_args(args, MACHINE.slot_bytes)),
};
