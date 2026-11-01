// @xpute/host/ipc/toc/key.spec.ts
//
// GENERATED from spec/ipc/toc.json

/** Where the submission ring's words begin. */
export const SQ = 1 as number;

/** Where the completion ring's words begin. */
export const CQ = 2 as number;

/** Where the hostcall stream's words begin (ipc/sys.json). */
export const SYS = 6 as number;

/** The bit every key a program numbers for itself carries: xpute gives none of
 * them a meaning, and a host that is not the program's passes over them. */
export const PROGRAM = 2147483648 as number;
