// @pxl/client/service/core/edit.ts

import type { i32, u32 } from "@xpute/kit/abi/word.ts";
import type { Tool } from "@/abi/edit.ts";
import * as edit from "@/abi/edit.ts";
import { core } from "@/service/core/core.ts";

/** Resolves once the core holds the tool, and rejects with the errno it
 * refused the tool with. */
export async function tool(
  tool: Tool,
  val: u32,
  scale_steps: i32,
): Promise<void> {
  await edit.hold(core.edit, tool, val, scale_steps);
}

/** Resolves once the core takes the text, and rejects when it refuses the
 * text whole, in which case the marker keeps the text it had. */
export async function marker_text(text: string): Promise<void> {
  await edit.marker_text(core.edit, text);
}

export function marker_commit(): void {
  void edit.marker_commit(core.edit);
}

export function marker_cancel(): void {
  void edit.marker_cancel(core.edit);
}
