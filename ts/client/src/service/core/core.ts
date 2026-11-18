// @pxl/client/service/core/core.ts

/** The core as the UI reaches it: the UI's end, which the core's interfaces
 * are called over by their named handles (`ui`). */

import type { Sender } from "@xpute/link/link.ts";

export let ui: Sender;

/** Makes `end` the UI's end, before the core boots. */
export function setUi(end: Sender): void {
  ui = end;
}
