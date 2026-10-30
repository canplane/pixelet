// @pxl/client/driver/visibility.ts

import { Command, encode_world_visibility } from "@/abi/cmd.spec.ts";
import { HOST, set_page_visible } from "@/host/mount.ts";
import * as port from "@/abi/port.spec.ts";

/** The platform's word that the page or the app is hidden or shown, heard from
 * boot (bracket.tsx `visibility`): rung at once with no quota, since a hidden
 * page gets no frame and hiding is the last sure moment for a save; and the
 * turns stop while it is hidden. */
export function world_visible(visible: boolean): void {
  HOST.doorbell.send(
    port.SHELL,
    Command.WORLD_VISIBILITY,
    encode_world_visibility(HOST.doorbell, visible),
  );
  HOST.doorbell.ring(0);
  set_page_visible(visible);
}
