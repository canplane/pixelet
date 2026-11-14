// @pxl/client/ui/shared/component/pan.ts

import type { ViewStyle } from "react-native";

/** A strip that scrolls sideways on a page: the browser scrolls it itself and
 * leaves holds and vertical drags to the view's own gestures. */
export const PAN_X: ViewStyle = { touchAction: "pan-x" } as ViewStyle;
