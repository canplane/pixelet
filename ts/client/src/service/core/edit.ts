// @pxl/client/service/core/edit.ts

import type { i32, u32 } from "@xpute/kit/abi/word.ts";
import type { Tool } from "@/abi/cmd.spec.ts";
import { core } from "@/service/core/core.ts";

/** Resolves to 0, or to a negative errno when the core refuses the tool. */
export function tool(tool: Tool, val: u32, scale_steps: i32): Promise<i32> {
  return core.edit_tool(tool, val, scale_steps);
}

/** Resolves to 0, or to a negative errno when the core refuses the text
 * whole, in which case the marker keeps the text it had. */
export function marker_text(text: string): Promise<i32> {
  return core.edit_marker_text(text);
}

export function marker_commit(): void {
  core.edit_marker_commit();
}

export function marker_cancel(): void {
  core.edit_marker_cancel();
}
