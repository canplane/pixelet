// @pxl/client/ui/shared/asset/swatch.tsx

import { Image, type StyleProp, View, type ViewStyle } from "react-native";

import type { u16 } from "@xpute/kit/abi/word.ts";
import {
  DECOR_BOAT,
  DECOR_CAR,
  DECOR_TRAIN,
  DECOR_TREE,
} from "@/ui/shared/asset/decor.spec.ts";
import { type Cube, cubeHex } from "@/ui/shared/asset/catalog.ts";
import { shippedTopTextureKey } from "@/ui/shared/asset/texture.ts";
import type { GlyphName } from "@/ui/shared/component/icon.tsx";
import { assetUrl, href } from "@/service/origin/route.ts";
import { SETTING } from "@/ui/shared/setting.ts";
import { useCore } from "@/ui/shared/watch.ts";

function cubeTextureUrl(cube: Cube): string | null {
  const key = shippedTopTextureKey(cube);
  return key ? href(assetUrl(key)) : null;
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
  const textureUrl = textured ? cubeTextureUrl(cube) : null;
  if (textureUrl) {
    return (
      <View style={[box, style]}>
        <Image
          source={{ uri: textureUrl }}
          resizeMode="cover"
          style={{ width: size, height: size }}
        />
      </View>
    );
  }
  return (
    <View
      style={[box, { backgroundColor: cubeHex(cube) ?? undefined }, style]}
    />
  );
}

const DECOR_GLYPH: Readonly<Record<number, GlyphName>> = {
  [DECOR_TREE]: "tree",
  [DECOR_BOAT]: "sailboat",
  [DECOR_CAR]: "car",
  [DECOR_TRAIN]: "train.side.front.car",
};

export function decorGlyph(id: u16): GlyphName {
  return DECOR_GLYPH[id];
}

export function selectionRing(
  ground: string,
  ink: string,
  width: number,
): string {
  return `0 0 0 1px ${ground}, 0 0 0 ${1 + width}px ${ink}`;
}
