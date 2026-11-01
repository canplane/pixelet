// @pxl/client/port/sensor.ts

/** What the shell asks of the sensors: the location, from the search panel's
 * card. */

import { Command } from "@/abi/cmd.spec.ts";
import { HOST } from "@/host/mount.ts";
import * as port from "@/abi/port.spec.ts";

export function kernel_locate(): void {
  HOST.now(port.SHELL, Command.SENSOR_LOCATE);
}
