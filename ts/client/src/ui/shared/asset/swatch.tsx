// @pxl/client/ui/shared/asset/swatch.tsx

import { Image, type StyleProp, View, type ViewStyle } from "react-native";

import type { u16 } from "@xpute/kit/abi/word.ts";
import {
  DECOR_BOAT,
  DECOR_CAR,
  DECOR_TRAIN,
  DECOR_TREE,
} from "@/ui/shared/asset/decor.spec.ts";
import { type Cube, cube_hex } from "@/ui/shared/asset/catalog.ts";
import { shipped_top_texture_key } from "@/ui/shared/asset/texture.ts";
import type { GlyphName } from "@/ui/shared/component/icon.tsx";
import { asset_url, href } from "@/service/origin/route.ts";
import { SETTING } from "@/ui/shared/setting.ts";
import { useCore } from "@/ui/shared/watch.ts";

function cube_texture_url(cube: Cube): string | null {
  const key = shipped_top_texture_key(cube);
  return key ? href(asset_url(key)) : null;
}

interface CubeSwatchProps {
  cube: Cube;
  size: number;
  radius: number;
  style?: StyleProp<ViewStyle>;
}

export function CubeSwatch({ cube, size, radius, style }: CubeSwatchProps) {
  const box = {
    width: size,
    height: size,
    borderRadius: radius,
    overflow: "hidden" as const,
  };
  const textured = useCore((s) => s.settings[SETTING.texture.key] === 1);
  const texture_url = textured ? cube_texture_url(cube) : null;
  if (texture_url) {
    return (
      <View style={[box, style]}>
        <Image
          source={{ uri: texture_url }}
          resizeMode="cover"
          style={{ width: size, height: size }}
        />
      </View>
    );
  }
  return (
    <View
      style={[box, { backgroundColor: cube_hex(cube) ?? undefined }, style]}
    />
  );
}

const DECOR_GLYPH: Readonly<Record<number, GlyphName>> = {
  [DECOR_TREE]: "tree",
  [DECOR_BOAT]: "sailboat",
  [DECOR_CAR]: "car",
  [DECOR_TRAIN]: "train.side.front.car",
};

export function decor_glyph(id: u16): GlyphName {
  return DECOR_GLYPH[id];
}

export function selection_ring(
  ground: string,
  ink: string,
  width: number,
): string {
  return `0 0 0 1px ${ground}, 0 0 0 ${1 + width}px ${ink}`;
}
