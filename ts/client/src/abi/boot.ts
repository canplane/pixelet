// @pxl/client/abi/boot.ts

/**
 * Boots the client, bottom up, on its platform's services (service/): the
 * runtime opened where its platform places it (service/runtime.ts, a page's
 * in the kernel's worker; service/runtime.native.ts, a device's beside the
 * shell), the kernel's commands bound to it, then where the boot stands as the
 * runtime starts, the GPU answers, the world's source is declared and the
 * kernel's snapshots come. The shell is no part of it: it reads where the
 * boot stands through the port (port/shell.ts), and what the kernel shows it
 * once it has started.
 */

import { Listeners } from "@xpute/kit/lang/listeners.ts";
import * as Errno from "@xpute/kit/status/errno.spec.ts";
import { InvariantError } from "@xpute/kit/status/error.ts";
import { GroundStatus, type Snapshot } from "@/abi/cmd.spec.ts";
import {
  BOOT_FAILED,
  BOOT_LOADING,
  BOOT_OPENING,
  BOOT_STARTED,
  BOOT_STARTING,
  BOOT_TERMINATED,
  type Boot,
} from "@/abi/state.ts";
import * as snapshot from "@/port/snapshot.ts";
import { open } from "@/service/runtime.ts";

let called = false;
let state: Boot = BOOT_OPENING;
const listeners = new Listeners();

function set(next: Boot): void {
  state = next;
  listeners.notify();
}

/** Where the boot stands, read through the port. */
export function boot_state(): Boot {
  return state;
}

export const watch_boot = listeners.watch;

/** Whether the world has come, or nothing will show of it: every pipeline made,
 * and the view drawn or past the service area. A failed location draws the view
 * at its default. */
function loaded(s: Snapshot): boolean {
  return (
    s.renderer_progress === 1 &&
    (s.pivot.status === GroundStatus.Success || s.pivot.outside)
  );
}

/** Called once by the app, ahead of the shell (src/boot.ts); where it stands
 * is `boot_state`. A second call is refused. */
export function boot(): void {
  if (called) throw new InvariantError(Errno.EALREADY);
  called = true;
  open({
    started: () => {
      if (state === BOOT_OPENING) set(BOOT_STARTING);
    },
    snapshot: () => {
      if (state === BOOT_LOADING && loaded(snapshot.current()))
        set(BOOT_STARTED);
    },
    // Its turns end with it.
    terminated: () => set(BOOT_TERMINATED),
  }).then(
    (declared) => {
      if (declared && state === BOOT_STARTING) set(BOOT_LOADING);
    },
    (err) => {
      console.error("[boot] the client did not start:", err);
      if (state === BOOT_OPENING || state === BOOT_STARTING) set(BOOT_FAILED);
    },
  );
}
