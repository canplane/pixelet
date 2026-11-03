// @pxl/client/port/setting.ts

/** The setting's command: one value set from the sheet or a picker. */

import type { u32 } from "@xpute/kit/abi/word.ts";
import { Command, encode_setting_set } from "@/abi/cmd.spec.ts";
import { HOST } from "@/host/mount.ts";
import * as port from "@/abi/port.spec.ts";

export function set(key: u32, value: u32): void {
  HOST.now(
    port.SHELL,
    Command.SETTING_SET,
    encode_setting_set(HOST.doorbell, key, value),
  );
}
