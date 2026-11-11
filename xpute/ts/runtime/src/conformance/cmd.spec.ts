// @xpute/runtime/conformance/cmd.spec.ts
//
// GENERATED from spec/conformance/core.json

/** Succeeds and replies 0. */
export const OK = 4097 as number;

/** Fails with EINVAL. */
export const FAIL = 4098 as number;

/** Raises as many NOTEs as the packet's number, tagged from 0 upward, then replies with
 * that number. */
export const RAISE = 4099 as number;

/** Writes one record to the stream, with the packet's number as its op and no words, and
 * replies 0. */
export const RECORD = 4100 as number;

/** Takes a handle from the core's table and replies with it. */
export const ISSUE = 4101 as number;

/** Gives back the packet's handle; fails with EBADF for one that is not live. */
export const RELEASE = 4102 as number;

/** Replies 0 for a live handle, and fails with EBADF for any other handle, 0 included. */
export const USE = 4103 as number;

/** Reads the runtime's clock and keeps the reading; replies 0. */
export const MARK = 4104 as number;

/** Reads the runtime's clock and replies with the whole milliseconds since the last MARK. */
export const SINCE = 4105 as number;

/** Opens the file (1, "f") on behalf of an owner, the packet's number, and replies with the
 * descriptor. The open's FS_OPEN is posted for the file device, which no trace's runtime
 * takes. When a trace completes the open (`complete`), the core raises OPEN_COMPLETION for
 * that owner if the descriptor is still open. */
export const OPEN = 4106 as number;

/** Closes the packet's descriptor, whether or not its open has completed; fails with EBADF
 * for a descriptor that is not open. */
export const CLOSE = 4107 as number;

/** Zeroes a word of its own, calls INC with the packet's number and the word's offset, and
 * replies with the word as it reads after the call. Where the runtime refuses INC, CALL
 * fails with the same errno. */
export const CALL = 4108 as number;

/** The call that CALL makes; its packet is a branch of a number and an offset. The runtime
 * writes the number plus one at the offset and answers 0 before the call returns. It
 * refuses a number of 0 with EINVAL and writes nothing. */
export const INC = 4109 as number;

/** Replies 0, its reply naming as many bytes of the core's own state as the packet's
 * number, byte k holding k, which go with the reply's frame; 0 names none. */
export const NAMED = 4110 as number;

/** The import that RAISE raises. */
export const NOTE = 4353 as number;

/** An open's completion, handed on. The frame is tagged with the open's owner, and its
 * result is what the runtime completed the open with. */
export const OPEN_COMPLETION = 4354 as number;
