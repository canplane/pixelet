// @pxl/web/platform/web/typeface.ts

import type { TextStyle } from "react-native";

/** The chrome's faces on a page: the system's own, as index.css names it,
 * and a monospace for numbers that have to line up. */
export const FAMILY: string | undefined = "-apple-system, BlinkMacSystemFont, system-ui, sans-serif";
export const MONO: string | undefined = 'ui-monospace, SFMono-Regular, Menlo, "Roboto Mono", monospace';

/** A thin edge of `tint` around text, so it reads over any ground: one CSS
 * value, since react-native-web warns on the three a device takes. */
export function textEdge(tint: string): TextStyle {
  return { textShadow: `0 0 2px ${tint}` } as TextStyle;
}
