// @pxl/native/platform/visual-viewport.ts

import { useSafeAreaInsets } from "react-native-safe-area-context";

/** Nothing to follow: a device's window does not scroll under its keyboard
 * the way a page's visual viewport does. */
export function useVisualViewport(): void {}

/** How far down the top overlays stand: under the notch. */
export function useViewportTop(): number {
  return useSafeAreaInsets().top;
}
