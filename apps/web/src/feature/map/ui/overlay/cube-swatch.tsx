// @pxl/web/feature/map/ui/overlay/cube-swatch.tsx

import { Image, type StyleProp, View, type ViewStyle } from "react-native";

import { type PlaceableCube, placeableCubeHex } from "@/catalog/cell/mod.ts";
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

/** A cube as a swatch — the real texture-pack top image when one exists,
 * falling back to a flat color otherwise (the same fallback hover_cursor.rs's
 * own ghost preview already relies on for object/tex.ts's own
 * get_object_texture). Shared by drawer-modal.tsx and mode-bar.tsx so their
 * swatches never drift apart. */
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
