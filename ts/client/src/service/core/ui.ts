// @pxl/client/service/core/ui.ts

/** The core as the UI reaches it besides each major's own commands. */

import type { u32 } from "@xpute/kit/abi/word.ts";
import type { NodeValue, NodeView } from "@xpute/kit/wire/xtp/mod.ts";
import { encode_args } from "@xpute/runtime";
import { MACHINE } from "@/abi/machine.spec.ts";
import { ui } from "@/service/core/core.ts";

/** The `cmd` of a development console. It sends any command by its number,
 * with its arguments as the fields of its packet, and rings the core. */
export const door = {
  cmd: (cmd: u32, ...args: NodeValue<NodeView>[]): void =>
    ui.send(cmd, encode_args(args, MACHINE.slot_bytes)),
};
