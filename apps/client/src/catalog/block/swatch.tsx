// @pxl/client/catalog/block/swatch.tsx

import { Image, type StyleProp, View, type ViewStyle } from "react-native";

import type { u8 } from "@xpute/kit/abi/word.ts";
import type { DecorId } from "@/catalog/block/decor.spec.ts";
import { type Cube, cubeHex, cubeId, decorName } from "@/catalog/block/mod.ts";
import { shippedTopTextureKey } from "@/catalog/asset/texture.ts";
import type { GlyphName } from "@platform/icon.tsx";
import { ORIGIN } from "@platform/origin.ts";
import { assetUrl } from "@/config/route.ts";

function cubeTextureUrl(cube: Cube): string | null {
  const key = shippedTopTextureKey(cubeId(cube));
  return key ? `${ORIGIN}${assetUrl(key)}` : null;
}

interface CubeSwatchProps {
  cube: Cube;
  size: number;
  radius: number;
  style?: StyleProp<ViewStyle>;
}

export function CubeSwatch({ cube, size, radius, style }: CubeSwatchProps) {
  const box = { width: size, height: size, borderRadius: radius, overflow: "hidden" as const };
  const textureUrl = cubeTextureUrl(cube);
  if (textureUrl) {
    return (
      <View style={[box, style]}>
        <Image source={{ uri: textureUrl }} resizeMode="cover" style={{ width: size, height: size }} />
      </View>
    );
  }
  return <View style={[box, { backgroundColor: cubeHex(cube) ?? undefined }, style]} />;
}

const DECOR_GLYPH: Record<DecorId, GlyphName> = {
  tree: "tree",
  boat: "sailboat",
  car: "car",
  train: "train.side.front.car",
};

export function decorGlyph(kind: u8): GlyphName {
  return DECOR_GLYPH[decorName(kind)];
}

export function cubeLabel(t: (key: string) => string, cube: Cube): string {
  return t(`blocks.cube.${cubeId(cube)}`);
}

export function decorLabel(t: (key: string) => string, kind: u8): string {
  return t(`blocks.decor.${decorName(kind)}`);
}

export function heldRing(ground: string, ink: string, width: number): string {
  return `0 0 0 1px ${ground}, 0 0 0 ${1 + width}px ${ink}`;
}
