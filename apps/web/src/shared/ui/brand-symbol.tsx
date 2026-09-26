// @pxl/web/shared/ui/brand-symbol.tsx

import { Path, Svg } from "react-native-svg";

import { SYMBOL_PATH, SYMBOL_VIEW_BOX } from "./symbol-path.ts";

/**
 * The brand's symbol, the cube, as one colour, taking a lucide icon's props — `size` a
 * side of its square and `color` its ink — so it stands where one of
 * lucide's would. The square is the cube's own height, with no room around
 * it as a lucide glyph leaves itself: `size` is how tall the cube is drawn.
 * The shape is the brand's (apps/web/brand/symbol-mono.svg), written here by
 * `deno task render:brand`.
 */
export function BrandSymbol({ size = 24, color = "currentColor" }: { size?: number; color?: string }) {
  return (
    <Svg width={size} height={size} viewBox={SYMBOL_VIEW_BOX}>
      <Path fill={color} fillRule="evenodd" d={SYMBOL_PATH} />
    </Svg>
  );
}
