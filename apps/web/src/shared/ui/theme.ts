// @pxl/web/shared/ui/theme.ts

/**
 * The shell's one theme, as values rather than CSS variables: a style made
 * from it is the same on the web, where react-native-web compiles it to
 * atomic classes, and on a device, where there is no CSS to hold a variable.
 * There is one theme, so this is a constant and not a context; a second one
 * is what would make it a hook.
 *
 * Colors are sRGB, named for what they are drawn on and as, not for a hue:
 * two uses that happen to share a value keep their own names, so changing
 * one never changes the other.
 */

import { FAMILY, MONO, textEdge } from "@platform/typeface.ts";

/** `alpha(color, 0.5)`: a `#rrggbb` at an opacity. */
export function alpha(hex: string, a: number): string {
  const n = parseInt(hex.slice(1), 16);
  return `rgba(${(n >> 16) & 255}, ${(n >> 8) & 255}, ${n & 255}, ${a})`;
}

export const color = {
  white: "#ffffff",
  black: "#000000",

  // The map's chrome, which floats over the world.
  /** Every label and glyph on the chrome: a tree-bark brown. */
  ink: "#5B4636",
  /** The round controls', the address pill's and the toasts' solid ground. */
  cream: "#FFFDF7",
  /** A round control switched on: its ground, and the glow of its ring. */
  lit: "#fffbeb",
  glow: "#ffb900",
  /** The follow mode's compass. */
  heading: "#fe9a00",
  /** The compass needle's north end. */
  north: "#fb2c36",
  /** The tool strip's flat dark. */
  band: "#171717",
  /** The send button. */
  send: "#ff6900",
  /** Something ready, as a sensor or the world is. */
  ready: "#00bc7d",
  /** A report that something failed: its mark, its ground and its edge. */
  alarm: "#fb2c36",
  alarmGround: "#fef2f2",
  alarmEdge: "#fecaca",

  // The panels' surface: a warm paper and its bark-brown text.
  paper: "#fcf8f1",
  text: "#272117",
  textMuted: "#696257",
  /** A warm wash for what is picked or pressed; used at an opacity. */
  wash: "#efe3cf",
  /** What is chosen on a panel: a radio, a switch, a chip. */
  chosen: "#4b361c",
  /** An off control's track, clearly darker than the paper. */
  track: "#d5cdc0",

  // What a setting costs the device, and what only a developer sees.
  costLow: "#009966",
  costMedium: "#e17100",
  costHigh: "#e7000b",
  dev: "#7f22fe",
  /** The ledger's buttons: one to begin a window, and the ink of one to
   * begin it and of one to stop it, over its dark panel. */
  ledgerGo: "#00d492",
  ledgerGoInk: "#5ee9b5",
  ledgerHaltInk: "#ffd230",
} as const;

/** A panel's hairlines: its text, faint. */
export const hairline = alpha(color.text, 0.1);

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
