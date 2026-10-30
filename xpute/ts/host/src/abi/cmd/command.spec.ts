// @xpute/host/abi/cmd/command.spec.ts
//
// GENERATED from spec/abi/cmd.json

// xpute's own commands.

/** Replies with its packet's value, or 0 where it has none — the rings' own test. */
export const NOP = 0x0000;
/** An OPENAT answered: the file's size, or -errno — ENOENT where there is no such file.
 * Never sent for a descriptor already closed. */
export const SYS_OPENED = 0x0101;
