// @pxl/web/kernel/shell.ts

import type { U8Array } from "@xpute/kit/abi/array.ts";
import type { u32 } from "@xpute/kit/abi/word.ts";
import { FaultError } from "@xpute/kit/status/error.ts";
import type { Errno } from "@xpute/kit/status/errno.spec.ts";
import { Command, encode_setting_set } from "@/kernel/abi/cmd.spec.ts";
import { GUEST, GUEST_DOORBELL } from "@/kernel/mem.ts";
import { ring_now } from "@/kernel/sched.ts";

export { kernel_snapshot, useKernel } from "@/kernel/snapshot.ts";
export { kernel_hold, kernel_keys } from "@/kernel/control/input.ts";
export { ring_now as kernel_command } from "@/kernel/sched.ts";

export function kernel_setting(key: u32, value: u32): void {
  ring_now(Command.SETTING_SET, encode_setting_set({ key, value }));
}

/** Whether a command carrying `pkt` can be sent; the shell refuses input that cannot. */
export function kernel_fits(pkt: U8Array): boolean {
  return GUEST_DOORBELL.fits(pkt);
}

export function kernel_enable_location(): void {
  ring_now(Command.SENSOR_ENABLE_LOCATION);
}

/** The kernel's memory, where a call leaves its reply; a view of it holds until the next turn. */
export function kernel_memory(): ArrayBuffer {
  return GUEST.buffer;
}

/** A command answered inside the call: the address of its reply in `kernel_memory()`. */
export function kernel_call(cmd: Command, pkt: U8Array | null = null): u32 {
  return GUEST_DOORBELL.call(cmd, pkt, (at) => {
    // A refusal is -errno, never an address.
    if (at < 0) throw new FaultError(-at as Errno);
    return at >>> 0;
  });
}
