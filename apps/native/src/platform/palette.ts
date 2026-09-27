// @pxl/native/platform/palette.ts

/**
 * The chrome's words and glyphs by what they are, on an iPhone: iOS's own
 * label and status colors, which the system keeps legible on its glass and
 * moves with dark mode and Increase Contrast.
 */

import { PlatformColor } from "react-native";

/** A system color, typed as the page's colors are. It is an object React
 * Native hands to the view as it is, and nothing here reads it; the one type
 * is what lets a view take it as it takes the page's, where react-native-svg
 * checks colors against another React Native's types than the app's. */
function system(name: string): string {
  return PlatformColor(name) as unknown as string;
}

export const LABEL: { primary: string; secondary: string; placeholder: string } = {
  primary: system("label"),
  secondary: system("secondaryLabel"),
  placeholder: system("placeholderText"),
};

export const STATUS: { ready: string; pending: string; failed: string } = {
  ready: system("systemGreen"),
  pending: system("systemOrange"),
  failed: system("systemRed"),
};
