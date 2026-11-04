// @pxl/client/port/sys.ts

/** SYS on the shell's port: the page's visibility. The boot's SYS_BOOT is
 * abi/boot.ts's. */

import { Command, encode_sys_sleep } from "@/abi/cmd.spec.ts";
import { HOST, set_page_visible } from "@/host/mount.ts";
import * as port from "@/abi/port.spec.ts";

/** The platform's word that the page or the app is hidden or shown, heard from
 * boot (bracket.tsx `visibility`): called at once, since a hidden page gets no
 * frame and hiding is the last sure moment for a save; and the turns stop
 * while it is hidden. */
export function world_visible(visible: boolean): void {
  HOST.call(
    port.SHELL,
    Command.SYS_SLEEP,
    encode_sys_sleep(HOST.doorbell, visible),
  );
  set_page_visible(visible);
}
