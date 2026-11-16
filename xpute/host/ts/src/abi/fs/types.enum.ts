// @xpute/host/abi/fs/types.enum.ts
//
// GENERATED from wit/fs (xpute-link-gen wit.rs)

export type ErrorCode =
  | typeof ERROR_CODE_NO_ENTRY
  | typeof ERROR_CODE_IO
  | typeof ERROR_CODE_READ_ONLY
  | typeof ERROR_CODE_ACCESS
  | typeof ERROR_CODE_BAD_MESSAGE
  | typeof ERROR_CODE_REMOTE_UNREACHABLE
  | typeof ERROR_CODE_NO_OBJECT;
/** No file is there. */
export const ERROR_CODE_NO_ENTRY = 0;
/** Its backend failed to read or write it. */
export const ERROR_CODE_IO = 1;
/** Its backend keeps nothing written there. */
export const ERROR_CODE_READ_ONLY = 2;
/** Its backend refused access to it. */
export const ERROR_CODE_ACCESS = 3;
/** Its backend holds it in a form it cannot read as bytes. */
export const ERROR_CODE_BAD_MESSAGE = 4;
/** The network did not reach its server. */
export const ERROR_CODE_REMOTE_UNREACHABLE = 5;
/** Its server holds no such object. */
export const ERROR_CODE_NO_OBJECT = 6;

/** The member `val` is, or null for a number no member has. */
export function of_ErrorCode(val: number): ErrorCode | null {
  return val === ERROR_CODE_NO_ENTRY ||
    val === ERROR_CODE_IO ||
    val === ERROR_CODE_READ_ONLY ||
    val === ERROR_CODE_ACCESS ||
    val === ERROR_CODE_BAD_MESSAGE ||
    val === ERROR_CODE_REMOTE_UNREACHABLE ||
    val === ERROR_CODE_NO_OBJECT
    ? val
    : null;
}

/** The member's name, as its source spells it. */
export function name_ErrorCode(val: ErrorCode): string {
  switch (val) {
    case ERROR_CODE_NO_ENTRY:
      return "NoEntry";
    case ERROR_CODE_IO:
      return "Io";
    case ERROR_CODE_READ_ONLY:
      return "ReadOnly";
    case ERROR_CODE_ACCESS:
      return "Access";
    case ERROR_CODE_BAD_MESSAGE:
      return "BadMessage";
    case ERROR_CODE_REMOTE_UNREACHABLE:
      return "RemoteUnreachable";
    case ERROR_CODE_NO_OBJECT:
      return "NoObject";
  }
}
