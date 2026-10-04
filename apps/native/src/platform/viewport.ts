// @pxl/native/platform/viewport.ts

import { useWindowDimensions } from "react-native";

import type { f64 } from "@xpute/core/abi/word.ts";

export function useWindowSize(): { width: f64; height: f64 } {
  const { width, height } = useWindowDimensions();
  return { width, height };
}

/** A device's window does not scroll under its keyboard. */
export function useVisualViewport(): void {}

export function useViewportTop(): number {
  return 0;
}
