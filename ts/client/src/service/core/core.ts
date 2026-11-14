// @pxl/client/service/core/core.ts

/** The core as the UI reaches it: the UI's remote end of the link (`ui`),
 * which the core's interfaces are called over by their named handles. */

import type { Remote } from "@xpute/link";

export let ui: Remote;

/** Makes `remote` the UI's end of the link, before the core boots. */
export function set_ui(remote: Remote): void {
  ui = remote;
}
