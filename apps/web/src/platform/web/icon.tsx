// @pxl/web/platform/web/icon.tsx

import {
  ArrowUp,
  Bug,
  Check,
  CircleArrowUp,
  Eraser,
  Eye,
  Gauge,
  Globe,
  Hammer,
  LayoutGrid,
  Loader,
  type LucideIcon,
  MessageSquare,
  Move3d,
  Navigation,
  Navigation2,
  Palette,
  Pointer,
  Search,
  Settings,
  Sun,
  TreeDeciduous,
  X,
} from "lucide-react-native";

import type { f64 } from "@xpute/core/abi/word.ts";
import { Spin } from "@platform/motion.tsx";

/**
 * The chrome's glyphs, named as an iPhone names them: the SF Symbols an
 * iPhone draws (apps/native/src/platform/icon.tsx), which the chrome's look
 * is held to. A page and an Android phone cannot draw those — Apple licenses
 * them to its own platforms alone — so each is drawn here as the lucide
 * glyph nearest it in shape, filled where the symbol is.
 */
const LUCIDE = {
  "arrow.up": [ArrowUp, false],
  "bubble.left": [MessageSquare, false],
  "checkmark": [Check, false],
  "hand.point.up.left": [Pointer, false],
  "eraser": [Eraser, false],
  "eye": [Eye, false],
  "gauge.medium": [Gauge, false],
  "gearshape": [Settings, false],
  "globe": [Globe, false],
  "hammer": [Hammer, false],
  "ladybug": [Bug, false],
  "location": [Navigation, false],
  "location.fill": [Navigation, true],
  "location.north.circle": [CircleArrowUp, false],
  "location.north.line.fill": [Navigation2, true],
  "magnifyingglass": [Search, false],
  "move.3d": [Move3d, false],
  "paintpalette": [Palette, false],
  "square.grid.2x2": [LayoutGrid, false],
  "sun.max": [Sun, false],
  "tree": [TreeDeciduous, false],
  "xmark": [X, false],
} as const satisfies Record<string, readonly [LucideIcon, boolean]>;

export type GlyphName = keyof typeof LUCIDE;

/** How heavy a glyph's strokes are, by SF Symbols' names for the weights. */
export type GlyphWeight = "regular" | "semibold" | "bold" | "heavy" | "black";

/** A lucide stroke, in its 24-unit box, for each weight. */
const STROKE: Record<GlyphWeight, f64> = { regular: 2, semibold: 2.5, bold: 2.75, heavy: 3, black: 3.5 };

export interface IconProps {
  glyph: GlyphName;
  /** The glyph's box, a side. */
  size: f64;
  color: string;
  weight?: GlyphWeight;
}

/** The stroke a glyph of `weight` is drawn with, in its box's units — what a
 * mark drawn beside it (shared/ui/glyph.tsx's strike) matches. */
export function strokeOf(weight: GlyphWeight = "regular"): f64 {
  return STROKE[weight];
}

export function Icon({ glyph, size, color, weight = "regular" }: IconProps) {
  const [Drawn, filled] = LUCIDE[glyph];
  return <Drawn size={size} color={color} strokeWidth={STROKE[weight]} fill={filled ? color : "none"} />;
}

/** What stands for a glyph while something is waited on: lucide's spokes,
 * which are an iPhone's activity indicator's shape, turned once a second. */
export function Spinner({ size, color }: { size: f64; color: string }) {
  return (
    <Spin periodMs={1000}>
      <Loader size={size} color={color} strokeWidth={STROKE.semibold} />
    </Spin>
  );
}
