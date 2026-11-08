// @pxl/client/ui/component/theme.ts

/**
 * The UI's one theme. Its colors and sizes are plain values rather than CSS
 * variables, so that a style is the same on the page and on a device.
 */

import { GROUND } from "@/ui/component/ground.ts";
import { round } from "@xpute/kit/math/scalar.ts";

export function alpha(hex: string, a: number): string {
  const n = parseInt(hex.slice(1), 16);
  return `rgba(${(n >> 16) & 255}, ${(n >> 8) & 255}, ${n & 255}, ${a})`;
}

/** Returns the solid color that `hex` at opacity `a` shows over `over`. */
function tint(hex: string, a: number, over = "#ffffff"): string {
  const n = parseInt(hex.slice(1), 16);
  const g = parseInt(over.slice(1), 16);
  const mix = (c: number, b: number) =>
    round(b + (c - b) * a)
      .toString(16)
      .padStart(2, "0");
  return `#${[16, 8, 0]
    .map((s) => mix((n >> s) & 255, (g >> s) & 255))
    .join("")}`;
}

/** iOS draws a secondary label in the label's color at this opacity, in light
 * and dark mode alike. */
export const SECONDARY = 0.6;

/** iOS's system colors, with their values in light mode. */
export const SYSTEM = {
  red: "#FF3B30",
  orange: "#FF9500",
  yellow: "#FFCC00",
  green: "#34C759",
  blue: "#007AFF",
  purple: "#AF52DE",
  dark: "#1C1C1E",
} as const;

/** `mark` is the coral of the brand mark. `base` and `deep` are that coral
 * darkened in OKLCH: `base` reaches 3:1 contrast against white, for filled
 * controls, and `deep` reaches 4.5:1, for words on the ground. */
const CORAL = {
  mark: "#FF865E",
  base: "#E9724B",
  deep: "#C6522A",
} as const;

/** iOS's fill gray, warmed to match the ground. */
const FILL = "#7d766c";

export const color = {
  white: "#ffffff",
  black: "#000000",
  ground: GROUND,
  /** The blue screen shown when the core stops. It is iOS's system blue,
   * darkened so that white words on it reach 4.5:1 contrast. */
  panic: "#0069DC",

  /** iOS's label gray in light mode. */
  labelGray: "#3C3C43",
  band: SYSTEM.dark,
  send: CORAL.base,
  /** The send button takes this color when it sets a marker's words, so that
   * it is not mistaken for sending a chat line. */
  confirm: SYSTEM.blue,
  /** The label that names who said a chat line. A chat line lies over the
   * world on its own dark edge, so the label takes the brand mark's coral
   * rather than a shade darkened for the light ground. */
  self: CORAL.mark,
  /** The status colors mixed toward white, for a sensor glyph drawn over the
   * world. */
  pendingGlyph: tint(SYSTEM.orange, 0.6),
  alarmGlyph: tint(SYSTEM.red, 0.6),

  fill: FILL,
  /** iOS's tertiary system fill. */
  faint: alpha(FILL, 0.12),
  chosen: CORAL.base,
  action: CORAL.deep,
  /** The track of a switch that is off, as iOS draws it. */
  track: tint(FILL, 0.16, GROUND),
  /** The backdrop behind a character drawn on its own surface. */
  backdrop: tint(FILL, 0.4, GROUND),

  costLow: SYSTEM.green,
  costMedium: SYSTEM.orange,
  costHigh: SYSTEM.red,
  dev: SYSTEM.purple,
  perfGo: SYSTEM.green,
  perfHalt: SYSTEM.yellow,
} as const;

/** Colors of a surface that an iPhone draws as system glass and the other
 * platforms draw solid. `glass` tints the glass; `fill` and the rest make up
 * the solid surface. */
export const surface = {
  chosen: {
    glass: alpha(CORAL.base, 0.25),
    fill: tint(CORAL.base, 0.1),
    edge: alpha(CORAL.base, 0.6),
  },
  alarm: {
    glass: alpha(SYSTEM.red, 0.15),
    fill: tint(SYSTEM.red, 0.06),
    edge: tint(SYSTEM.red, 0.3),
    shadow: alpha(SYSTEM.red, 0.15),
  },
} as const;

export interface Material {
  opacity: number;
  blur: number;
  /** The fill's opacity over an iPhone's system blur. It is lower than
   * `opacity` because the system blur already tints what it blurs. */
  system: number;
}

export const material = {
  popover: { opacity: 0.6, blur: 8, system: 0.6 },
  /** iOS's bar, as Apple's own web pages draw it. */
  bar: { opacity: 0.72, blur: 20, system: 0.5 },
} as const satisfies Record<string, Material>;

/** The opacity of a disabled button's content. As on iOS, only the content
 * dims, never the glass under it. */
export const disabledContent = 0.4;

export const shadow = { lift: "0 6px 20px rgba(0, 0, 0, 0.25)" } as const;

/** A dark halo that lets white text read over any ground. It is drawn as
 * black copies of the text, grown by 1 to `width` pixels on each side, each at
 * opacity `ring`. */
export const halo = { width: 2, ring: 0.3 } as const;

export const haloRings: readonly number[] = Array.from(
  { length: halo.width },
  (_, i) => halo.width - i,
);

export const hairline = alpha(color.labelGray, 0.1);

export const weight = {
  regular: "400",
  medium: "500",
  semibold: "600",
} as const;

/** Apple's text styles, each a step smaller, because the chrome floats over
 * the world. */
export const textStyle = {
  title2: {
    fontSize: 20,
    lineHeight: 25,
    fontWeight: weight.semibold,
  },
  title: {
    fontSize: 15,
    lineHeight: 20,
    fontWeight: weight.semibold,
  },
  body: {
    fontSize: 15,
    lineHeight: 20,
    fontWeight: weight.regular,
  },
  /** 16 rather than 15: iOS Safari zooms in on a focused field whose text is
   * smaller than 16. */
  input: { fontSize: 16, fontWeight: weight.regular },
  footnote: {
    fontSize: 13,
    lineHeight: 18,
    fontWeight: weight.regular,
  },
  /** The words of a notice, such as a toast or an item's name. */
  notice: {
    fontSize: 13,
    lineHeight: 18,
    fontWeight: weight.semibold,
    letterSpacing: -0.3,
  },
  caption: {
    fontSize: 12,
    lineHeight: 16,
    fontWeight: weight.regular,
  },
  /** 11 is the smallest size iOS sets text in. */
  caption2: {
    fontSize: 11,
    lineHeight: 13,
    fontWeight: weight.medium,
  },
} as const;

/** The most a text grows with the system's text size. A control's text grows
 * to 1.2×. A mark, such as a setting's badge, does not grow, because its size
 * is its shape. Running text has no entry here and follows the setting in
 * full. */
export const textScale = { chrome: 1.2, mark: 1 } as const;

export const sectionTitle = {
  ...textStyle.caption,
  fontWeight: weight.medium,
  letterSpacing: 0.6,
  textTransform: "uppercase",
} as const;

/** The world screen's layers, back to front. `bottom` holds the dock and the
 * toasts, and `chrome` the controls along the top. */
export const layer = {
  bottom: 40,
  scrim: 49,
  chrome: 50,
  debug: 51,
  sheet: 70,
} as const;

export const chrome = {
  inset: 16,
  control: 48,
  controlGap: 8,
  /** The smallest touch target Apple's guidelines allow. */
  target: 44,
  glyph: 20,
  selection: 36,
} as const;
