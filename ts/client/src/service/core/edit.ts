// @pxl/client/service/core/edit.ts

/** The edit's commands: the tool and what it places, and the marker that
 * waits for its words. */

import type { i32, u32 } from "@xpute/kit/abi/word.ts";
import type { Tool } from "@/abi/cmd.spec.ts";
import { core } from "@/service/core/core.ts";

/** Tells the core what the pointer holds. Resolves to 0, or to a negative
 * errno when the core refuses it. */
export function tool(tool: Tool, val: u32, scale_steps: i32): Promise<i32> {
  return core.edit_tool(tool, val, scale_steps);
}

/** Gives the waiting marker its text as typed, and the marker is drawn with
 * it. Resolves to 0, or to a negative errno when the core refuses the text
 * whole, in which case the text before it stays. */
export function marker_text(text: string): Promise<i32> {
  return core.edit_marker_text(text);
}

export function marker_commit(): void {
  core.edit_marker_commit();
}

export function marker_cancel(): void {
  core.edit_marker_cancel();
}
