// @pxl/client/port/edit.ts

/** The edit's commands: the tool and what it takes, and the marker it left
 * waiting. */

import type { u32 } from "@xpute/kit/abi/word.ts";
import {
  type ActiveTool,
  Command,
  encode_edit_marker_text,
  encode_edit_tool,
} from "@/abi/cmd.spec.ts";
import { HOST } from "@/host/mount.ts";
import * as port from "@/abi/port.spec.ts";
import { send_input } from "./input.ts";

export function kernel_tool(tool: ActiveTool, val: u32, tier: u32): void {
  send_input(
    Command.EDIT_TOOL,
    encode_edit_tool(HOST.doorbell, tool, val, tier),
  );
}

/** The waiting marker's text as typed, which it is drawn with. */
export function kernel_marker_text(text: string): void {
  HOST.now(
    port.SHELL,
    Command.EDIT_MARKER_TEXT,
    encode_edit_marker_text(HOST.doorbell, text),
  );
}

/** The waiting marker committed with its text. */
export function kernel_marker_commit(): void {
  HOST.now(port.SHELL, Command.EDIT_MARKER_COMMIT);
}

/** The waiting marker let go. */
export function kernel_marker_cancel(): void {
  HOST.now(port.SHELL, Command.EDIT_MARKER_CANCEL);
}
