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
  LayoutGrid,
  Loader,
  MapPin,
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
  User,
  X,
} from "lucide-react-native";

import type { f64 } from "@xpute/core/abi/word.ts";
import { Spin } from "@platform/motion.tsx";

/** SF Symbol names, drawn as the nearest lucide glyph: Apple licenses the symbols to its own platforms only. */
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
  "ladybug": [Bug, false],
  "location": [Navigation, false],
  "location.fill": [Navigation, true],
  "location.north.circle": [CircleArrowUp, false],
  "location.north.line.fill": [Navigation2, true],
  "magnifyingglass": [Search, false],
  "mappin": [MapPin, false],
  "move.3d": [Move3d, false],
  "paintpalette": [Palette, false],
  "person.fill": [User, true],
  "square.grid.2x2": [LayoutGrid, false],
  "sun.max": [Sun, false],
  "tree": [TreeDeciduous, false],
  "xmark": [X, false],
} as const satisfies Record<string, readonly [LucideIcon, boolean]>;

export type GlyphName = keyof typeof LUCIDE;

export type GlyphWeight = "regular" | "semibold" | "bold" | "heavy" | "black";

const STROKE: Record<GlyphWeight, f64> = { regular: 2, semibold: 2.5, bold: 2.75, heavy: 3, black: 3.5 };

export interface IconProps {
  glyph: GlyphName;
  size: f64;
  color: string;
  weight?: GlyphWeight;
}

export function strokeOf(weight: GlyphWeight = "regular"): f64 {
  return STROKE[weight];
}

export function Icon({ glyph, size, color, weight = "regular" }: IconProps) {
  const [Drawn, filled] = LUCIDE[glyph];
  return <Drawn size={size} color={color} strokeWidth={STROKE[weight]} fill={filled ? color : "none"} />;
}

export function Spinner({ size, color }: { size: f64; color: string }) {
  return (
    <Spin periodMs={1000}>
      <Loader size={size} color={color} strokeWidth={STROKE.semibold} />
    </Spin>
  );
}
