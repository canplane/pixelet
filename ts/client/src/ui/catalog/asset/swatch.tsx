// @pxl/client/ui/catalog/asset/swatch.tsx

import { Image, type StyleProp, View, type ViewStyle } from "react-native";

import type { u8 } from "@xpute/kit/abi/word.ts";
import { decor } from "@/ui/catalog/asset/decor.spec.ts";
import { type Cube, cubeHex } from "@/ui/catalog/asset/mod.ts";
import { shippedTopTextureKey } from "@/ui/catalog/asset/texture.ts";
import type { GlyphName } from "@/ui/component/icon.tsx";
import { asset_url, href } from "@/port/ui.ts";
import { SETTING } from "@/ui/setting/schema.ts";
import { useCore } from "@/ui/store/watch.ts";

function cubeTextureUrl(cube: Cube): string | null {
  const key = shippedTopTextureKey(cube);
  return key ? href(asset_url(key)) : null;
}

interface CubeSwatchProps {
  cube: Cube;
  size: number;
  radius: number;
  style?: StyleProp<ViewStyle>;
}

/** A cube drawn as the world draws it: in its texture while the texture pack
 * is on, and in its color otherwise. */
export function CubeSwatch({ cube, size, radius, style }: CubeSwatchProps) {
  const box = {
    width: size,
    height: size,
    borderRadius: radius,
    overflow: "hidden" as const,
  };
  const textured = useCore((s) => s.settings[SETTING.textures.key] === 1);
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
  [decor.tree]: "tree",
  [decor.boat]: "sailboat",
  [decor.car]: "car",
  [decor.train]: "train.side.front.car",
};

export function decorGlyph(kind: u8): GlyphName {
  return DECOR_GLYPH[kind];
}

export function selectionRing(
  ground: string,
  ink: string,
  width: number,
): string {
  return `0 0 0 1px ${ground}, 0 0 0 ${1 + width}px ${ink}`;
}
