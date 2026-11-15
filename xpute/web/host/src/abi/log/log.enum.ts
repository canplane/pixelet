// @xpute/web-host/abi/log/log.enum.ts
//
// GENERATED from wit/log (xpute-link-gen wit.rs)

export type Level =
  | typeof LEVEL_TRACE
  | typeof LEVEL_DEBUG
  | typeof LEVEL_INFO
  | typeof LEVEL_WARN
  | typeof LEVEL_ERROR
  | typeof LEVEL_CRITICAL;
export const LEVEL_TRACE = 0;
export const LEVEL_DEBUG = 1;
export const LEVEL_INFO = 2;
export const LEVEL_WARN = 3;
export const LEVEL_ERROR = 4;
export const LEVEL_CRITICAL = 5;

/** The member `val` is, or null for a number no member has. */
export function of_Level(val: number): Level | null {
  return val === LEVEL_TRACE ||
    val === LEVEL_DEBUG ||
    val === LEVEL_INFO ||
    val === LEVEL_WARN ||
    val === LEVEL_ERROR ||
    val === LEVEL_CRITICAL
    ? val
    : null;
}
