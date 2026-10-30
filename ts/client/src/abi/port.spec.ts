// @pxl/client/abi/port.spec.ts
//
// GENERATED from spec/client/kernel/door/port.json
//
// pixelet's ports on xpute's rings (xpute spec/ipc/frame.json PORT_SHIFT): the number a
// frame names at the far side, the host being 0. 1 and 255 are the core memory's own,
// and 2 to 254 are kept for workers, each lent its slabs for a job. The client sends as
// the shell, and the kernel raises its events for it (pxl-kernel door.rs,
// ts/client/src/abi/boot.ts).

import type { u32 } from "@xpute/kit/abi/word.ts";

/** The core, the guest that holds the core memory: what a worker names to reach it. */
export const CORE: u32 = 1 as number;

/** The shell, with the drivers that bring the outside in (ts/client/src/port/,
 * ts/client/src/driver/). */
export const SHELL: u32 = 255 as number;
