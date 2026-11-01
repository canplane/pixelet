// @xpute/host/conformance/cmd.spec.ts
//
// GENERATED from spec/conformance/guest.json

/** Succeeds; replies 0. */
export const OK = 4097 as number;

/** Fails with EINVAL. */
export const FAIL = 4098 as number;

/** Raises the packet's number of EVENTs, tagged 0 up, then replies with that number. */
export const RAISE = 4099 as number;

/** Writes one record to the stream, its op the packet's number and no words; replies 0. */
export const RECORD = 4100 as number;

/** Takes a handle from the guest's table and replies with it. */
export const ISSUE = 4101 as number;

/** Gives back the packet's handle; fails with EBADF for one that is not live. */
export const RELEASE = 4102 as number;

/** Replies 0 for a live handle; fails with EBADF for any other, 0 among them. */
export const USE = 4103 as number;

/** Reads the host's clock and keeps the reading; replies 0. */
export const MARK = 4104 as number;

/** Reads the host's clock and replies with the whole milliseconds since the last MARK. */
export const SINCE = 4105 as number;

/** Opens a file for the packet's number, its owner, and replies with the descriptor; the host's answer (SYS_OPENED) raises OPENED for that owner while the descriptor is open. */
export const OPEN = 4106 as number;

/** Closes the packet's descriptor, answered or not; fails with EBADF for one not open. */
export const CLOSE = 4107 as number;

/** Zeroes a word of its own, makes the hostcall ANSWER of the packet's number and the word's offset, and replies with the word as it then reads it; fails with the host's errno where the host refused. */
export const HOSTCALL = 4108 as number;

/** What RAISE raises. */
export const EVENT = 4353 as number;

/** An open answered: tagged with its owner, its result the host's. */
export const OPENED = 4354 as number;
