// @pxl/web/platform/web/gesture.ts

import type { ViewStyle } from "react-native";

/** A view inside a strip that scrolls sideways: on a page the browser pans
 * the strip itself, and what is left of a finger — a hold, a drag up —
 * reaches the view's own gesture. */
export const PAN_X: ViewStyle = { touchAction: "pan-x" } as ViewStyle;
