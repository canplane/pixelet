// @pxl/web/shared/ui/theme.ts

/**
 * The shell's one theme, as values rather than CSS variables, so a style is
 * the same on the web and on a device. Colors are named for their use, not
 * their hue, so two uses sharing a value can change apart.
 */

import { FAMILY, MONO } from "@platform/typeface.ts";
import { round } from "@xpute/core/math/scalar.ts";

export function alpha(hex: string, a: number): string {
  const n = parseInt(hex.slice(1), 16);
  return `rgba(${(n >> 16) & 255}, ${(n >> 8) & 255}, ${n & 255}, ${a})`;
}

/** A `#rrggbb` at an opacity over a ground, as one solid color. */
function tint(hex: string, a: number, over = "#ffffff"): string {
  const n = parseInt(hex.slice(1), 16);
  const g = parseInt(over.slice(1), 16);
  const mix = (c: number, b: number) => round(b + (c - b) * a).toString(16).padStart(2, "0");
  return `#${[16, 8, 0].map((s) => mix((n >> s) & 255, (g >> s) & 255)).join("")}`;
}

/** iOS's system hues, in light mode. */
export const SYSTEM = {
  red: "#FF3B30",
  orange: "#FF9500",
  yellow: "#FFCC00",
  green: "#34C759",
  purple: "#AF52DE",
  dark: "#1C1C1E",
} as const;

/** The mark's coral (#FF865E) darkened in OKLCH to reach 3:1 against white
 * (`base`, filled controls) and 4.5:1 (`deep`, words on the ground). */
const CORAL = {
  base: "#E9724B",
  deep: "#C6522A",
} as const;

/** iOS's fill gray, turned as warm as the ground. */
const FILL = "#7d766c";

const GROUND = "#fffcf7";

export const color = {
  white: "#ffffff",
  black: "#000000",
  ground: GROUND,

  /** iOS's label gray in light mode. */
  labelGray: "#3C3C43",
  band: SYSTEM.dark,
  send: CORAL.base,
  self: CORAL.deep,
  /** The status hues lightened toward white, for a sensor glyph over the map. */
  pendingGlyph: tint(SYSTEM.orange, 0.6),
  alarmGlyph: tint(SYSTEM.red, 0.6),

  fill: FILL,
  /** iOS's tertiary system fill. */
  faint: alpha(FILL, 0.12),
  chosen: CORAL.base,
  action: CORAL.deep,
  /** iOS's off-switch track: the fill gray at 16% over the ground. */
  track: tint(FILL, 0.16, GROUND),

  costLow: SYSTEM.green,
  costMedium: SYSTEM.orange,
  costHigh: SYSTEM.red,
  dev: SYSTEM.purple,
  profilerGo: SYSTEM.green,
  profilerHalt: SYSTEM.yellow,
} as const;

/** Colors an iPhone draws on its system glass and other platforms as a
 * solid surface: `glass` is the tint, `fill` and the rest the solid look. */
export const surface = {
  chosen: { glass: alpha(CORAL.base, 0.25), fill: tint(CORAL.base, 0.1), edge: alpha(CORAL.base, 0.6) },
  alarm: { glass: alpha(SYSTEM.red, 0.15), fill: tint(SYSTEM.red, 0.06), edge: tint(SYSTEM.red, 0.3), shadow: alpha(SYSTEM.red, 0.15) },
} as const;

export interface Material {
  opacity: number;
  blur: number;
  /** Lower than `opacity` because an iPhone's blur tints what it blurs itself. */
  system: number;
}

export const material = {
  flyout: { opacity: 0.6, blur: 8, system: 0.6 },
  /** iOS's bar, as Apple's own page draws one. */
  bar: { opacity: 0.72, blur: 20, system: 0.5 },
} as const satisfies Record<string, Material>;

/** As iOS dims a disabled button: the content only, never the glass. */
export const disabledContent = 0.4;

export const shadow = { lift: "0 6px 20px rgba(0, 0, 0, 0.25)" } as const;

/** A dark halo so white reads over any ground: black copies grown by 1 to
 * `reach` px a side, each at opacity `ring`. */
export const halo = { reach: 2, ring: 0.3 } as const;

export const haloRings: readonly number[] = Array.from({ length: halo.reach }, (_, i) => halo.reach - i);

export const hairline = alpha(color.labelGray, 0.1);

export const font = {
  mono: MONO,
};

/** Apple's text styles, a step smaller, since the chrome floats over a map. */
export const weight = { regular: "400", medium: "500", semibold: "600" } as const;

export const textStyle = {
  title2: { fontFamily: FAMILY, fontSize: 20, lineHeight: 25, fontWeight: weight.semibold },
  title: { fontFamily: FAMILY, fontSize: 15, lineHeight: 20, fontWeight: weight.semibold },
  body: { fontFamily: FAMILY, fontSize: 15, lineHeight: 20, fontWeight: weight.regular },
  /** 16, not 15: iOS Safari zooms into a focused field set smaller. */
  input: { fontFamily: FAMILY, fontSize: 16, fontWeight: weight.regular },
  footnote: { fontFamily: FAMILY, fontSize: 13, lineHeight: 18, fontWeight: weight.regular },
  caption: { fontFamily: FAMILY, fontSize: 12, lineHeight: 16, fontWeight: weight.regular },
  /** 11 is iOS's floor. */
  caption2: { fontFamily: FAMILY, fontSize: 11, lineHeight: 13, fontWeight: weight.medium },
} as const;

/** How far a text follows the system's text size: a control's to 1.2×, a mark
 * not at all, since its size is its shape; running text follows it whole. */
export const textScale = { chrome: 1.2, mark: 1 } as const;

export const sectionTitle = { ...textStyle.caption, fontWeight: weight.medium, letterSpacing: 0.6, textTransform: "uppercase" } as const;

/** The map screen's layers, back to front: the dock and toasts, the scrim, the top chrome, a sheet. */
export const layer = { bottom: 40, scrim: 49, chrome: 50, sheet: 70 } as const;

export const chrome = {
  inset: 16,
  control: 48,
  controlGap: 8,
  /** iOS's least touch target (decision/20260928-three-sizes-of-a-control.md). */
  target: 44,
  glyph: 20,
  selection: 36,
} as const;
