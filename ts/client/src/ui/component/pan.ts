// @pxl/client/ui/component/pan.ts

import type { ViewStyle } from "react-native";

/** A strip that scrolls sideways on a page: the browser scrolls it itself and
 * leaves holds and vertical drags to the view's own gestures. A device's
 * responder system shares the finger between them (pan.native.ts). */
export const panX: ViewStyle = { touchAction: "pan-x" } as ViewStyle;
