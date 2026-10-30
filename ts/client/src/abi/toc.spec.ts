// @pxl/client/abi/toc.spec.ts
//
// GENERATED from spec/client/kernel/door/toc.json
//
// The kernel's own entries in its toc (xpute-guest ipc/toc.rs), each a
// key with xpute's PROGRAM bit set: the memory the host reads besides the rings and
// the hostcalls (pxl-kernel door.rs, ts/client/src/abi/boot.ts).

/** What an entry's value is. */
export const key = {
  /** Where the renderer's stream is, run after every turn; 0 where the kernel draws itself. */
  GPU_STREAM: 2147483650 as number,
} as const;
