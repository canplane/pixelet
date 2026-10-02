// @pxl/web/platform/web/typeface.ts

import type { TextStyle } from "react-native";

export const FAMILY: string | undefined = "-apple-system, BlinkMacSystemFont, system-ui, sans-serif";
export const MONO: string | undefined = 'ui-monospace, SFMono-Regular, Menlo, "Roboto Mono", monospace';

/** One CSS value: react-native-web warns on the three a device takes. */
export function textEdge(tint: string): TextStyle {
  return { textShadow: `0 0 2px ${tint}` } as TextStyle;
}
