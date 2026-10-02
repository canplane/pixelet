// @pxl/native/platform/viewport.ts

import { useWindowDimensions } from "react-native";
import { useSafeAreaInsets } from "react-native-safe-area-context";

import type { f64 } from "@xpute/core/abi/word.ts";

export interface WindowSize {
  width: f64;
  height: f64;
}

export function useWindowSize(): WindowSize {
  const { width, height } = useWindowDimensions();
  return { width, height };
}

/** A device's window does not scroll under its keyboard. */
export function useVisualViewport(): void {}

export function useViewportTop(): number {
  return useSafeAreaInsets().top;
}
