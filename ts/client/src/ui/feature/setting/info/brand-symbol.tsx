// @pxl/client/ui/feature/setting/info/brand-symbol.tsx

import { Path, Svg } from "react-native-svg";

import { SYMBOL_PATH, SYMBOL_VIEW_BOX } from "./symbol-path.ts";

/** `size` is the cube's height, with no padding. */
export function BrandSymbol({
  size = 24,
  color = "currentColor",
}: {
  size?: number;
  color?: string;
}) {
  return (
    <Svg width={size} height={size} viewBox={SYMBOL_VIEW_BOX}>
      <Path fill={color} fillRule="evenodd" d={SYMBOL_PATH} />
    </Svg>
  );
}
