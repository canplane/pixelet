// @pxl/web/shared/ui/logo.tsx

import { Path, Svg } from "react-native-svg";

import { LOGO_PATH, LOGO_VIEW_BOX } from "./logo-path.ts";

/**
 * The brand's cube as one colour, taking a lucide icon's props — `size` a
 * side of its square and `color` its ink — so it stands where one of
 * lucide's would. The square is the cube's own height, with no room around
 * it as a lucide glyph leaves itself: `size` is how tall the cube is drawn.
 * The shape is the brand's (apps/web/brand/logo.svg), written here by
 * `deno task render:brand`.
 */
export function Logo({ size = 24, color = "currentColor" }: { size?: number; color?: string }) {
  return (
    <Svg width={size} height={size} viewBox={LOGO_VIEW_BOX}>
      <Path fill={color} fillRule="evenodd" d={LOGO_PATH} />
    </Svg>
  );
}
