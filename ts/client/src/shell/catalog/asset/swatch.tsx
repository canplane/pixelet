// @pxl/client/shell/catalog/asset/swatch.tsx

import { Image, type StyleProp, View, type ViewStyle } from "react-native";

import type { u8 } from "@xpute/kit/abi/word.ts";
import type { DecorId } from "@/shell/catalog/asset/decor.spec.ts";
import {
  type Cube,
  cubeHex,
  cubeId,
  decorName,
} from "@/shell/catalog/asset/mod.ts";
import { shippedTopTextureKey } from "@/shell/catalog/asset/texture.ts";
import type { GlyphName } from "@/shell/ui/icon.tsx";
import { asset_url } from "@/port/shell.ts";
import { SETTING } from "@/shell/setting/schema.ts";
import { bracket } from "@/bracket.tsx";
import { useKernel } from "@/shell/store/watch.ts";

function cubeTextureUrl(cube: Cube): string | null {
  const key = shippedTopTextureKey(cubeId(cube));
  return key ? `${bracket.origin.url}${asset_url(key)}` : null;
}

interface CubeSwatchProps {
  cube: Cube;
  size: number;
  radius: number;
  style?: StyleProp<ViewStyle>;
}

/** The cube as the world draws it: its texture while the pack is on, else its color. */
export function CubeSwatch({ cube, size, radius, style }: CubeSwatchProps) {
  const box = {
    width: size,
    height: size,
    borderRadius: radius,
    overflow: "hidden" as const,
  };
  const textured = useKernel((s) => s.settings[SETTING.textures.key] === 1);
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
  return t(`assets.cube.${cubeId(cube)}`);
}

export function decorLabel(t: (key: string) => string, kind: u8): string {
  return t(`assets.decor.${decorName(kind)}`);
}

export function heldRing(ground: string, ink: string, width: number): string {
  return `0 0 0 1px ${ground}, 0 0 0 ${1 + width}px ${ink}`;
}
