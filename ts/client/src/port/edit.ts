// @pxl/client/port/edit.ts

/** The edit's commands: the tool and what it takes, and the marker it left
 * waiting. */

import type { i32, u32 } from "@xpute/kit/abi/word.ts";
import type { Tool } from "@/abi/cmd.spec.ts";
import { kernel } from "@/host/kernel.ts";

/** What the pointer holds, told to the kernel: 0, or a negative errno it refused with. */
export function tool(tool: Tool, val: u32, scale_steps: i32): i32 {
  return kernel.edit_tool(tool, val, scale_steps);
}

/** The waiting marker's text as typed, which it is drawn with: 0, or a negative
 * errno where the kernel refused it whole, the text before staying. */
export function marker_text(text: string): i32 {
  return kernel.edit_marker_text(text);
}

/** The waiting marker committed with its text. */
export function marker_commit(): void {
  kernel.edit_marker_commit();
}

/** The waiting marker let go. */
export function marker_cancel(): void {
  kernel.edit_marker_cancel();
}
