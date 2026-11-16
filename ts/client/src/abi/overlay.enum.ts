// @pxl/client/abi/overlay.enum.ts
//
// GENERATED from spec/client/core/wit (xpute-link-gen wit.rs)

export type Message =
  typeof MESSAGE_WORLD_READ_FAILING | typeof MESSAGE_WORLD_UNAVAILABLE;
/** Reads reach the server and fail; they are tried again. */
export const MESSAGE_WORLD_READ_FAILING = 0;
/** Reads failed past every retry; the world is not coming. */
export const MESSAGE_WORLD_UNAVAILABLE = 1;

/** The member `val` is, or null for a number no member has. */
export function of_Message(val: number): Message | null {
  return val === MESSAGE_WORLD_READ_FAILING || val === MESSAGE_WORLD_UNAVAILABLE
    ? val
    : null;
}

/** The member's name, as its source spells it. */
export function name_Message(val: Message): string {
  switch (val) {
    case MESSAGE_WORLD_READ_FAILING:
      return "WorldReadFailing";
    case MESSAGE_WORLD_UNAVAILABLE:
      return "WorldUnavailable";
  }
}
