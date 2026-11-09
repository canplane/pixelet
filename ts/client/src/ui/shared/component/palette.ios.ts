// @pxl/client/ui/shared/component/palette.ios.ts

/** iOS's label and status colors, which follow dark mode and Increase Contrast. */

import { PlatformColor } from "react-native";

/** A PlatformColor is an object. It is typed here as a string so that
 * react-native-svg's color props accept it. */
function system(name: string): string {
  return PlatformColor(name) as unknown as string;
}

export const LABEL: {
  primary: string;
  secondary: string;
  tertiary: string;
  placeholder: string;
} = {
  primary: system("label"),
  secondary: system("secondaryLabel"),
  tertiary: system("tertiaryLabel"),
  placeholder: system("placeholderText"),
};

export const NORTH: string = system("systemRed");

export const STATUS: { ready: string; pending: string; failed: string } = {
  ready: system("systemGreen"),
  pending: system("systemOrange"),
  failed: system("systemRed"),
};
