// @pxl/web/catalog/cell/swatch.tsx

import { Image, type StyleProp, View, type ViewStyle } from "react-native";

import type { u8 } from "@xpute/core/abi/word.ts";
import type { DecorId } from "@/catalog/cell/decor.spec.ts";
import { decorName, type PlaceableCube, placeableCubeChannel, placeableCubeHex, placeableCubeId, placeableCubeType } from "@/catalog/cell/mod.ts";
import { shippedTopTextureKey } from "@/catalog/asset/texture.ts";
import type { GlyphName } from "@platform/icon.tsx";
import { ORIGIN } from "@platform/origin.ts";
import { assetUrl } from "@/config/route.ts";

function placeableCubeTextureUrl(cube: PlaceableCube): string | null {
  const key = shippedTopTextureKey(placeableCubeChannel(cube), placeableCubeType(cube));
  return key ? `${ORIGIN}${assetUrl(key)}` : null;
}

interface CubeSwatchProps {
  cube: PlaceableCube;
  size: number;
  radius: number;
  style?: StyleProp<ViewStyle>;
}

export function CubeSwatch({ cube, size, radius, style }: CubeSwatchProps) {
  const box = { width: size, height: size, borderRadius: radius, overflow: "hidden" as const };
  const textureUrl = placeableCubeTextureUrl(cube);
  if (textureUrl) {
    return (
      <View style={[box, style]}>
        <Image source={{ uri: textureUrl }} resizeMode="cover" style={{ width: size, height: size }} />
      </View>
    );
  }
  return <View style={[box, { backgroundColor: placeableCubeHex(cube) ?? undefined }, style]} />;
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

export function cubeLabel(t: (key: string) => string, cube: PlaceableCube): string {
  return t(`cubes.${placeableCubeId(cube)}`);
}

export function decorLabel(t: (key: string) => string, kind: u8): string {
  return t(`decor.${decorName(kind)}`);
}

export function heldRing(ground: string, ink: string, width: number): string {
  return `0 0 0 1px ${ground}, 0 0 0 ${1 + width}px ${ink}`;
}
