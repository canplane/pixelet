// @pxl/native/platform/palette.ts

/**
 * The chrome's words and glyphs by what they are, on a device: iOS's own
 * label and status colors, which the system keeps legible on its glass and
 * moves with dark mode and Increase Contrast. Android names its colors
 * otherwise, and takes the page's ink until it has its own here.
 */

import { Platform, PlatformColor } from "react-native";

import { alpha, color } from "@/shared/ui/theme.ts";

const IOS = Platform.OS === "ios";

/** A system color, typed as the page's colors are. It is an object React
 * Native hands to the view as it is, and nothing here reads it; the one type
 * is what lets a view take it as it takes the page's, where react-native-svg
 * checks colors against another React Native's types than the app's. */
function system(name: string): string {
  return PlatformColor(name) as unknown as string;
}

export const LABEL: { primary: string; secondary: string; placeholder: string } = IOS
  ? { primary: system("label"), secondary: system("secondaryLabel"), placeholder: system("placeholderText") }
  : { primary: color.ink, secondary: alpha(color.ink, 0.5), placeholder: alpha(color.ink, 0.4) };

export const STATUS: { ready: string; pending: string; failed: string } = IOS
  ? { ready: system("systemGreen"), pending: system("systemOrange"), failed: system("systemRed") }
  : { ready: color.ready, pending: color.glow, failed: color.alarm };
