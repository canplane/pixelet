// @pxl/client/abi/edit.enum.ts
//
// GENERATED from spec/client/core/wit (xpute-link-gen wit.rs)

export type Tool = typeof TOOL_POINTER | typeof TOOL_ITEM | typeof TOOL_ERASER;
export const TOOL_POINTER = 0;
export const TOOL_ITEM = 1;
export const TOOL_ERASER = 2;

/** The member `val` is, or null for a number no member has. */
export function of_Tool(val: number): Tool | null {
  return val === TOOL_POINTER || val === TOOL_ITEM || val === TOOL_ERASER
    ? val
    : null;
}

/** The member's name, as its source spells it. */
export function name_Tool(val: Tool): string {
  switch (val) {
    case TOOL_POINTER:
      return "Pointer";
    case TOOL_ITEM:
      return "Item";
    case TOOL_ERASER:
      return "Eraser";
  }
}
