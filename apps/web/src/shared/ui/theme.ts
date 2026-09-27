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

/** `tint(color, 0.1)`: a `#rrggbb` laid over white at an opacity, as one
 * solid color — for a ground that must not show what is behind it. */
function tint(hex: string, a: number): string {
  const n = parseInt(hex.slice(1), 16);
  const mix = (c: number) => Math.round(255 + (c - 255) * a).toString(16).padStart(2, "0");
  return `#${mix((n >> 16) & 255)}${mix((n >> 8) & 255)}${mix(n & 255)}`;
}

/** iOS's system hues, in light mode: what a state or a cost is drawn in,
 * at an opacity or as a tint, so it reads as the system's own would. A
 * device takes the system's own where it can (platform palette.ts), which
 * also moves with dark mode. */
const SYSTEM = {
  red: "#FF3B30",
  orange: "#FF9500",
  yellow: "#FFCC00",
  green: "#34C759",
  purple: "#AF52DE",
  /** The dark mode's grouped ground. */
  dark: "#1C1C1E",
} as const;

/** The app's own hue: the coral of its mark (#FF865E), darkened with its hue
 * and chroma held (OKLCH) to where white on it, or it on white, reaches a
 * contrast — 3:1 for a filled control and its white glyph or word, 4.5:1 for
 * words in the coral on the paper. The mark's own is lighter than either,
 * too light to carry white. Everything the app itself says — chosen, send,
 * one's own — is this, and nothing a state or a cost says. */
const CORAL = {
  base: "#E9724B",
  deep: "#C6522A",
} as const;

/** The gray the fills on a sheet are drawn at an opacity of: iOS's fill gray
 * turned a little warm, as the paper is. */
const FILL = "#7d766c";

export const color = {
  white: "#ffffff",
  black: "#000000",

  // The map's chrome, which floats over the world.
  /** iOS's label gray in light mode, the base its secondary and tertiary
   * labels are drawn at an opacity of; the page's labels are iOS's
   * (platform palette.ts), and a hairline is this gray, faint. */
  labelGray: "#3C3C43",
  /** The round controls' and the toasts' solid ground, where there is no
   * system glass: the paper's warm white. */
  ground: "#fffcf7",
  /** The profiler's warning. */
  glow: SYSTEM.yellow,
  /** The compass's north on the page (platform palette.ts `NORTH`): the
   * system's red, as iOS's own Maps and Compass draw it. */
  north: SYSTEM.red,
  /** The send button. */
  send: CORAL.base,
  /** One's own name in the chat. */
  self: CORAL.deep,
  /** A status: something ready, as a sensor or the world is; something on
   * its way; a report that something failed. */
  ready: SYSTEM.green,
  pending: SYSTEM.orange,
  alarm: SYSTEM.red,
  /** A sensor's glyph over the map, on its way and failed: the status hues
   * lightened toward the white of the glyphs beside them by one ratio, so
   * they tell without shouting and stay a pair. */
  pendingGlyph: tint(SYSTEM.orange, 0.6),
  alarmGlyph: tint(SYSTEM.red, 0.6),

  // The panels' surface and what stands on it: iOS's system colors, each
  // turned a little warm, so the sheets keep the app's warmth under the
  // system's words.
  /** A sheet's ground, on the page and on a device's sheet alike: white,
   * barely warm. */
  paper: "#fffcf7",
  /** For what is picked or pressed; used at an opacity. */
  wash: FILL,
  /** Background of unselected controls (tracks, chips, round buttons and
   * fields on a sheet): iOS's tertiary system fill, in the warm gray.
   * Selected controls use a solid fill instead. */
  faint: alpha(FILL, 0.12),
  /** What is chosen on a panel: a check, a switch, a chip. */
  chosen: CORAL.base,
  /** An action's words on a panel. */
  action: CORAL.deep,
  /** An off switch's track: iOS's, in the warm gray over the paper. */
  track: "#ebe7e0",

  // What a setting costs the device, and what only a developer sees.
  costLow: SYSTEM.green,
  costMedium: SYSTEM.orange,
  costHigh: SYSTEM.red,
  dev: SYSTEM.purple,
  /** The profiler's buttons: one to begin a window, and the ink of one to
   * begin it and of one to stop it, over its dark panel. */
  profilerGo: SYSTEM.green,
  profilerGoInk: SYSTEM.green,
  profilerHaltInk: SYSTEM.yellow,
} as const;

/**
 * The colors a device with the system's glass draws differently from the
 * page and Android, which have none: each is one thing drawn two ways, side
 * by side here, so where the platforms part shows in this one place. `glass`
 * is what the glass is tinted or holds; `fill` and the rest are the solid
 * surface drawn where there is no glass.
 */
export const surface = {
  /** A control switched on: the app's coral, as whatever is chosen is. */
  on: { glass: alpha(CORAL.base, 0.25), fill: tint(CORAL.base, 0.1), edge: alpha(CORAL.base, 0.6) },
  /** A report that something failed. */
  alarm: { glass: alpha(SYSTEM.red, 0.15), fill: tint(SYSTEM.red, 0.06), edge: tint(SYSTEM.red, 0.3), shadow: alpha(SYSTEM.red, 0.15) },
} as const;

/** The tool strip and its size flyout, in the two sets they are drawn in: a
 * toolbar on the system's glass, and a dark band where there is none
 * (feature/map/ui/overlay/mode-bar.tsx says why). */
export const strip = {
  glass: {
    glyph: color.black,
    glyphOn: color.black,
    held: alpha(color.black, 0.08),
    line: alpha(color.black, 0.1),
    ink: color.black,
    ground: color.white,
    flyout: color.white,
  },
  band: {
    glyph: alpha(color.white, 0.6),
    glyphOn: color.white,
    held: alpha(color.white, 0.2),
    line: alpha(color.white, 0.15),
    ink: color.white,
    ground: SYSTEM.dark,
    flyout: alpha(SYSTEM.dark, 0.8),
  },
} as const;

/** The soft dark halo what floats over the map stands in (the address, the
 * glyphs beside it, the scale), so white reads over any ground, as a map's
 * labels do: black copies of a glyph, a line or a text grown by each of
 * `rings` pixels a side, each copy faded whole to `ring`, so the dark falls
 * off with the distance from the stroke (shared/ui/halo.tsx). */
export const halo = { rings: [2, 1], ring: 0.3 } as const;

/** A panel's hairlines: its text, faint. */
export const hairline = alpha(color.labelGray, 0.1);

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
  /** A panel's heading: body size, semibold, like an iOS sheet title. */
  title: { fontFamily: FAMILY, fontSize: 15, lineHeight: 20, fontWeight: weight.semibold },
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
