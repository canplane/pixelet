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

/** `tint(color, 0.1)`: a `#rrggbb` laid over a ground (white, or `over`) at
 * an opacity, as one solid color — for a ground that must not show what is
 * behind it. */
function tint(hex: string, a: number, over = "#ffffff"): string {
  const n = parseInt(hex.slice(1), 16);
  const g = parseInt(over.slice(1), 16);
  const mix = (c: number, b: number) => Math.round(b + (c - b) * a).toString(16).padStart(2, "0");
  return `#${[16, 8, 0].map((s) => mix((n >> s) & 255, (g >> s) & 255)).join("")}`;
}

/** iOS's system hues, in light mode: what a state or a cost is drawn in,
 * at an opacity or as a tint, so it reads as the system's own would. The
 * page's palette names its roles from these; a device names the system's own
 * (platform palette.ts). */
export const SYSTEM = {
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
 * words in the coral on the ground. The mark's own is lighter than either,
 * too light to carry white. Everything the app itself says — chosen, send,
 * one's own — is this, and nothing a state or a cost says. */
const CORAL = {
  base: "#E9724B",
  deep: "#C6522A",
} as const;

/** The gray a sheet's fills are drawn in at an opacity: iOS's fill gray,
 * turned as warm as the ground. */
const FILL = "#7d766c";

/** White, barely warm: what the sheets and the solid controls stand on. */
const GROUND = "#fffcf7";

export const color = {
  white: "#ffffff",
  black: "#000000",
  /** Every light surface: a sheet and a card on it, and where there is no
   * system glass, the round controls, the message pill and the toasts. One
   * color, so a control reads as the same stuff as the sheet it opens. */
  ground: GROUND,

  // The map's chrome, which floats over the world.
  /** iOS's label gray in light mode, the base its secondary and tertiary
   * labels are drawn at an opacity of; the page's labels are iOS's
   * (platform palette.ts), and a hairline is this gray, faint. */
  labelGray: "#3C3C43",
  /** The tool strip's dark band, where there is no system glass. */
  band: SYSTEM.dark,
  /** The send button. */
  send: CORAL.base,
  /** One's own name in the chat. */
  self: CORAL.deep,
  /** A sensor's glyph over the map, on its way and failed: the status hues
   * lightened toward the white of the glyphs beside them by one ratio, so
   * they tell without shouting and stay a pair. */
  pendingGlyph: tint(SYSTEM.orange, 0.6),
  alarmGlyph: tint(SYSTEM.red, 0.6),

  // The panels' surface and what stands on it: iOS's system colors, turned
  // barely warm.
  /** The fill gray, for a ground an opacity of it is set on. */
  fill: FILL,
  /** Background of unselected controls (tracks, chips, round buttons and
   * fields on a sheet): iOS's tertiary system fill. Selected controls use a
   * solid fill instead. */
  faint: alpha(FILL, 0.12),
  /** What is chosen on a panel: a check, a switch, a chip. */
  chosen: CORAL.base,
  /** An action's words on a panel. */
  action: CORAL.deep,
  /** An off switch's track: iOS's, its fill gray at 16% over the ground. */
  track: tint(FILL, 0.16, GROUND),

  // What a setting costs the device, and what only a developer sees.
  costLow: SYSTEM.green,
  costMedium: SYSTEM.orange,
  costHigh: SYSTEM.red,
  dev: SYSTEM.purple,
  /** The profiler's, over its dark panel: the button that begins a window,
   * and the one that stops it, which a gauge's warning shares. */
  profilerGo: SYSTEM.green,
  profilerHalt: SYSTEM.yellow,
} as const;

/**
 * The colors a device with the system's glass draws differently from the
 * page and Android, which have none: each is one thing drawn two ways, side
 * by side here, so where the platforms part shows in this one place. `glass`
 * is what the glass is tinted or holds; `fill` and the rest are the solid
 * surface drawn where there is no glass.
 */
export const surface = {
  /** A control switched on: the coral of whatever is chosen, and a ring of it
   * around the control. */
  chosen: { glass: alpha(CORAL.base, 0.25), fill: tint(CORAL.base, 0.1), edge: alpha(CORAL.base, 0.6) },
  /** A report that something failed. */
  alarm: { glass: alpha(SYSTEM.red, 0.15), fill: tint(SYSTEM.red, 0.06), edge: tint(SYSTEM.red, 0.3), shadow: alpha(SYSTEM.red, 0.15) },
} as const;

/** A see-through backing (@platform backdrop.ts): how much of its fill
 * stands over what is behind it, blurred as far as `blur` where the platform
 * can; where it cannot, the fill stands solid. What a platform does with it
 * is its own; each surface keeps its own recipe, as the system's materials
 * do. */
export interface Material {
  opacity: number;
  blur: number;
  /** The fill's opacity over an iPhone's blur, which tints what it blurs
   * itself: less than `opacity`, which is over a blur alone. */
  system: number;
}

export const material = {
  /** The size flyout over the map. */
  flyout: { opacity: 0.6, blur: 8, system: 0.6 },
  /** A sheet's header, over its content scrolling under it: iOS's bar, as
   * Apple's own page draws one, 72% over a 20px blur. */
  bar: { opacity: 0.72, blur: 20, system: 0.5 },
} as const satisfies Record<string, Material>;

/** A control that takes no press: what it holds at this opacity, its ground
 * as it is, as iOS dims a disabled button — the glass left whole, which an
 * opacity or a clear style would read as the button gone rather than off. */
export const disabledContent = 0.4;

/** A floating control's lift off the map. */
export const shadow = { lift: "0 6px 20px rgba(0, 0, 0, 0.25)" } as const;

/** The soft dark halo what floats over the map stands in (the address, the
 * glyphs beside it, the scale), so white reads over any ground, as a map's
 * labels do: black copies of a glyph, a line or a text grown by 1 to `reach`
 * pixels a side, each copy faded whole to `ring`, so the dark falls off with
 * the distance from the stroke (shared/ui/halo.tsx). */
export const halo = { reach: 2, ring: 0.3 } as const;

/** The halo's rings, one per pixel of its reach, widest first. */
export const haloRings: readonly number[] = Array.from({ length: halo.reach }, (_, i) => halo.reach - i);

/** A panel's hairlines: its text, faint. */
export const hairline = alpha(color.labelGray, 0.1);

/** A face by name: the platform's (platform/web/typeface.ts, a device's
 * beside it) — a CSS stack a page falls back through, or one face a device
 * has and its own system face where none is named. */
export const font = {
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
  /** A round map control's side, and the gap between two controls side by
   * side or stacked. */
  control: 48,
  controlGap: 8,
  /** The least a press lands on, whatever the control one sees: iOS's 44
   * (issue/20260928-three-sizes-of-a-control.md). */
  target: 44,
  /** A control's glyph, one size whatever the control's: a map control's 48,
   * a sheet's 44, a slot in the strip. */
  glyph: 20,
  /** What is lit in a row of choices 44 tall (the strip's tool in hand, a
   * segmented control's segment): a capsule this tall inside its slot. */
  selection: 36,
} as const;
