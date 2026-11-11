// @pxl/client/service/core/core.ts

/** The core as the client reaches it once it has started: its commands,
 * bound to the UI's link (`ui`). They are set once, before anything uses
 * them. */

import type { Link } from "@xpute/runtime";
import { type UiExports, ui_exports } from "@/abi/cmd.spec.ts";

export let core: UiExports;
export let ui: Link;

export function set_core(link: Link): void {
  core = ui_exports(link);
  ui = link;
}
