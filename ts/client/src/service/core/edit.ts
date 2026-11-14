// @pxl/client/service/core/edit.ts

import type { i32, u32 } from "@xpute/kit/abi/word.ts";
import { ComponentError } from "@xpute/link";
import type { Tool } from "@/abi/edit.enum.ts";
import * as edit from "@/abi/edit.ts";
import { ui } from "@/service/core/core.ts";

/** Resolves once the core holds the tool, and rejects with the errno it
 * refused the tool with. */
export async function tool(
  tool: Tool,
  val: u32,
  scale_step: i32,
): Promise<void> {
  await edit.hold(ui, tool, val, scale_step);
}

/** Resolves whether the core took the text: false where it is longer than a
 * marker holds, and the marker keeps the text it had. It rejects where the
 * link refuses the text whole. */
export async function marker_text(text: string): Promise<boolean> {
  return edit.marker_text(ui, text).then(
    () => true,
    (e: unknown) => {
      if (!(e instanceof ComponentError)) throw e;
      return false;
    },
  );
}

export function marker_commit(): void {
  void edit.marker_commit(ui);
}

export function marker_cancel(): void {
  void edit.marker_cancel(ui);
}
