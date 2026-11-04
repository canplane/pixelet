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

/** The kernel, the guest that holds the core memory: what a worker names to reach it. */
export const KERNEL: u32 = 1 as number;

/** The shell, the UI, on the place's event loop: what the user does, and what the
 * place's sensors, GPU, visibility and frames tell it (ts/client/src/port/). */
export const SHELL: u32 = 255 as number;
