// @pxl/client/shell/ui/brand-symbol.tsx

import { Path, Svg } from "react-native-svg";

import { SYMBOL_PATH, SYMBOL_VIEW_BOX } from "./symbol-path.ts";

/** The brand's cube in one color, with an icon's props; `size` is the
 * cube's height, with no padding. Written by `./script/tool.ts render brand`. */
export function BrandSymbol({ size = 24, color = "currentColor" }: { size?: number; color?: string }) {
  return (
    <Svg width={size} height={size} viewBox={SYMBOL_VIEW_BOX}>
      <Path fill={color} fillRule="evenodd" d={SYMBOL_PATH} />
    </Svg>
  );
}
