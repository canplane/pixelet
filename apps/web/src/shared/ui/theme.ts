// @pxl/web/shared/ui/theme.ts

/**
 * The shell's one theme, as values rather than CSS variables: a style made
 * from it is the same on the web, where react-native-web compiles it to
 * atomic classes, and on a device, where there is no CSS to hold a variable.
 * There is one theme, so this is a constant and not a context; a second one
 * is what would make it a hook.
 *
 * Colors are sRGB. React Native reads no oklch, so the palette's entries are
 * Tailwind v4's own oklch values converted once.
 */

import { FAMILY, MONO, textEdge } from "@platform/typeface.ts";

/** `alpha(color, 0.5)`: a `#rrggbb` at an opacity, as the palette's `/50`. */
export function alpha(hex: string, a: number): string {
  const n = parseInt(hex.slice(1), 16);
  return `rgba(${(n >> 16) & 255}, ${(n >> 8) & 255}, ${n & 255}, ${a})`;
}

export const color = {
  /** The chrome's ink: a tree-bark brown, what every label and glyph is. */
  ink: "#5B4636",
  /** The round controls' and the address pill's solid fill. */
  cream: "#FFFDF7",
  amber400: "#ffb900",
  amber500: "#fe9a00",
  emerald500: "#00bc7d",
  red400: "#ff6467",
  red500: "#fb2c36",
  orange500: "#ff6900",
  /** The tool strip's flat dark. */
  neutral900: "#171717",
  white: "#ffffff",
  black: "#000000",

  // The panels' surface: a warm off-white and a bark-brown ink.
  background: "#fcf8f1",
  foreground: "#272117",
  mutedForeground: "#696257",
  /** A warm wash for what is picked or pressed; used at an opacity. */
  accent: "#efe3cf",
  primary: "#4b361c",
  /** An off control's track, clearly darker than the background. */
  input: "#d5cdc0",

  emerald600: "#009966",
  amber600: "#e17100",
  red600: "#e7000b",
  violet500: "#8e51ff",
  violet600: "#7f22fe",
} as const;

/** A panel's hairlines: the foreground, faint. */
export const hairline = alpha(color.foreground, 0.1);

/** A face by name: the platform's (platform/web/typeface.ts, a device's
 * beside it) — a CSS stack a page falls back through, or one face a device
 * has and its own system face where none is named. */
export const font = {
  family: FAMILY,
  /** For numbers that have to line up. */
  mono: MONO,
};

/** A thin dark edge around text, so it reads over any ground: the
 * platform's, which spell a text shadow differently. */
export { textEdge };

/** The map's floating chrome. */
export const chrome = {
  /** The inset from the screen's edge. */
  inset: 16,
  /** A round map control's side, and the gap between two stacked. */
  control: 48,
  controlGap: 10,
} as const;
