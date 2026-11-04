// @pxl/client/port/edit.ts

/** The edit's commands: the tool and what it takes, and the marker it left
 * waiting. */

import type { i32, u32 } from "@xpute/kit/abi/word.ts";
import * as Errno from "@xpute/kit/status/errno.spec.ts";
import {
  type Tool,
  Command,
  encode_edit_marker_text,
  encode_edit_tool,
} from "@/abi/cmd.spec.ts";
import { HOST } from "@/host/mount.ts";
import * as port from "@/abi/port.spec.ts";

/** What the pointer holds, told to the kernel: 0, or a negative errno it refused with. */
export function tool(tool: Tool, val: u32, scale_steps: i32): i32 {
  return HOST.call(
    port.SHELL,
    Command.EDIT_TOOL,
    encode_edit_tool(HOST.doorbell, tool, val, scale_steps),
    (result) => result,
  );
}

/** The waiting marker's text as typed, which it is drawn with: 0, or a negative
 * errno where the kernel refused it whole, the text before staying. */
export function marker_text(text: string): i32 {
  const pkt = encode_edit_marker_text(HOST.doorbell, text);
  if (!HOST.doorbell.fits(pkt)) return Errno.EMSGSIZE;
  return HOST.call(
    port.SHELL,
    Command.EDIT_MARKER_TEXT,
    pkt,
    (result) => result,
  );
}

/** The waiting marker committed with its text. */
export function marker_commit(): void {
  HOST.call(port.SHELL, Command.EDIT_MARKER_COMMIT);
}

/** The waiting marker let go. */
export function marker_cancel(): void {
  HOST.call(port.SHELL, Command.EDIT_MARKER_CANCEL);
}
