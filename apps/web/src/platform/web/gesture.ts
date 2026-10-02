// @pxl/web/platform/web/gesture.ts

import type { ViewStyle } from "react-native";

/** The browser pans the strip; holds and drags up reach the view's own gesture. */
export const PAN_X: ViewStyle = { touchAction: "pan-x" } as ViewStyle;
