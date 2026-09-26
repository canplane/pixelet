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
  /** A status, in Apple's own green, orange and red: something ready, as a
   * sensor or the world is; something on its way; a report that something
   * failed, with the failure's ground and edge. */
  ready: "#34C759",
  pending: "#FF9500",
  alarm: "#FF3B30",
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
  /** The profiler's buttons: one to begin a window, and the ink of one to
   * begin it and of one to stop it, over its dark panel. */
  profilerGo: "#00d492",
  profilerGoInk: "#5ee9b5",
  profilerHaltInk: "#ffd230",
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

/** The shell's text styles: Apple's, by name and by ratio, a step smaller,
 * since the chrome floats over a map and is kept small. Every text takes one
 * of these, and a weight only where its use differs from the style's: 400
 * for what is read, 500 for the chrome and a control, 600 for a title and a
 * toast. */
export const weight = { regular: "400", medium: "500", semibold: "600" } as const;

export const textStyle = {
  /** A panel's heading. */
  title: { fontFamily: FAMILY, fontSize: 17, lineHeight: 22, fontWeight: weight.semibold },
  /** What is read: a row's name, a result, a notice. */
  body: { fontFamily: FAMILY, fontSize: 15, lineHeight: 20, fontWeight: weight.regular },
  /** A text field. Sixteen, not the body's fifteen: iOS Safari zooms the page
   * into a field set smaller when it takes focus. */
  input: { fontFamily: FAMILY, fontSize: 16, fontWeight: weight.regular },
  /** A control's label, a toast, a short line. */
  footnote: { fontFamily: FAMILY, fontSize: 13, lineHeight: 18, fontWeight: weight.regular },
  /** Small print and the chrome's labels. */
  caption: { fontFamily: FAMILY, fontSize: 12, lineHeight: 16, fontWeight: weight.regular },
  /** The smallest label there is: eleven is iOS's floor. */
  caption2: { fontFamily: FAMILY, fontSize: 11, lineHeight: 13, fontWeight: weight.medium },
} as const;

/** A section's title over a list: the caption, set apart by case. */
export const sectionTitle = { ...textStyle.caption, fontWeight: weight.medium, letterSpacing: 0.6, textTransform: "uppercase" } as const;

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
