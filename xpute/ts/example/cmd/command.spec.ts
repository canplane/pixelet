// ts/example/cmd/command.spec.ts
//
// GENERATED from spec/example/cmd.json

// The example's commands.

/** Two numbers, answered with their sum and the number the host holds. */
export const SUM = 0x1001;
/** Raised as SUM is answered: the two numbers' sum, before the host's is added. */
export const SUMMED = 0x1002;
/** A file's path, read whole by a task; answered at once, its count raised as COUNTED once read. */
export const COUNT = 0x1003;
/** Raised once COUNT's file is read: its bytes, or -errno. */
export const COUNTED = 0x1004;
