// @pxl/web/feature/map/ui/overlay/cube-swatch.tsx

import { Image, type StyleProp, View, type ViewStyle } from "react-native";

import type { DecorId } from "@/catalog/cell/decor.spec.ts";
import { type PlaceableCube, placeableCubeHex, placeableCubeId } from "@/catalog/cell/mod.ts";
import type { GlyphName } from "@platform/icon.tsx";
import { ORIGIN } from "@platform/origin.ts";
import { assetUrl } from "@/config/route.ts";
import { textureKey } from "@/feature/map/object/tex.ts";

function placeableCubeTextureUrl(cube: PlaceableCube): string | null {
  const key = textureKey(cube);
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

export const DECOR_GLYPH: Record<DecorId, GlyphName> = {
  tree: "tree",
};

export function cubeLabel(t: (key: string) => string, cube: PlaceableCube): string {
  return t(`cubes.${placeableCubeId(cube)}`);
}

export function heldRing(ground: string, ink: string, width: number): string {
  return `0 0 0 1px ${ground}, 0 0 0 ${1 + width}px ${ink}`;
}
