// @pxl/client/abi/directory.spec.ts
//
// GENERATED from spec/client/kernel/door/directory.json
//
// The kernel's own entries in its directory (xpute-guest ipc/directory.rs), each a
// key with xpute's PROGRAM bit set: the memory the host reads besides the rings and
// the system calls (pxl-kernel door.rs, ts/client/src/abi/boot.ts).

/** What an entry's value is. */
export const key = {
  /** Where the renderer's stream is, run after every turn; 0 where the kernel draws itself. */
  GPU_STREAM: 2147483650 as number,
} as const;
