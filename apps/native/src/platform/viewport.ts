// @pxl/native/platform/viewport.ts

import { useWindowDimensions } from "react-native";
import { useSafeAreaInsets } from "react-native-safe-area-context";

import type { f64 } from "@xpute/core/abi/word.ts";

export interface WindowSize {
  width: f64;
  height: f64;
}

/** The window the chrome is laid out in, which a keyboard leaves as it is. */
export function useWindowSize(): WindowSize {
  const { width, height } = useWindowDimensions();
  return { width, height };
}

/** Nothing to follow: a device's window does not scroll under its keyboard
 * the way a page's visual viewport does. */
export function useVisualViewport(): void {}

/** How far down the top overlays stand: under the notch. */
export function useViewportTop(): number {
  return useSafeAreaInsets().top;
}
