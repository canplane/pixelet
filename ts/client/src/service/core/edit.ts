// @pxl/client/service/core/edit.ts

import type { i32, u32 } from "@xpute/kit/abi/word.ts";
import { ERRNO_EMSGSIZE } from "@xpute/kit/status/errno.spec.ts";
import { XputeError } from "@xpute/kit/status/error.ts";
import { ComponentError } from "@xpute/link/link.ts";
import type { Tool } from "@/abi/edit.enum.ts";
import * as edit from "@/abi/edit.ts";
import { ui } from "@/service/core/core.ts";

/** Resolves once the core holds the tool, and rejects with the errno it
 * refused the tool with. */
export async function tool(
  tool: Tool,
  value: u32,
  scaleStep: i32,
): Promise<void> {
  await edit.hold(ui, tool, value, scaleStep);
}

/** Sets the draft's text; where it is too long, refused by the core or,
 * past a frame, by the link (EMSGSIZE), the draft keeps the text it had. */
export function markerText(text: string): void {
  void edit.marker_text(ui, text).catch((e: unknown) => {
    if (e instanceof ComponentError) return;
    if (e instanceof XputeError && e.errnum === ERRNO_EMSGSIZE) return;
    throw e;
  });
}

export function markerCommit(): void {
  void edit.marker_commit(ui);
}

export function markerCancel(): void {
  void edit.marker_cancel(ui);
}
