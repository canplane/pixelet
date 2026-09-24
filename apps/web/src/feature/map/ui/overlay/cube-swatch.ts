// @pxl/web/feature/map/ui/overlay/cube-swatch.ts

import type { CSSProperties } from "react";
import { type PlaceableCube, placeableCubeHex } from "@/catalog/cell/mod.ts";
import { asset_url } from "@/config/route.ts";
import { textureKey } from "@/feature/map/object/tex.ts";

function placeableCubeTextureUrl(cube: PlaceableCube): string | null {
  const key = textureKey(cube, "top");
  return key ? asset_url(key) : null;
}

/** Style for a cube swatch — the real texture-pack top image when one
 * exists, falling back to a flat color swatch otherwise (the same fallback
 * hover_cursor.rs's own ghost preview already relies on for
 * object/tex.ts's own get_object_texture). Shared by drawer-modal.tsx and
 * mode-bar.tsx so their swatches never drift apart. */
export function cubeSwatchStyle(cube: PlaceableCube): CSSProperties {
  const textureUrl = placeableCubeTextureUrl(cube);
  if (textureUrl) {
    return { backgroundImage: `url(${textureUrl})`, backgroundSize: "cover", imageRendering: "pixelated" };
  }
  return { backgroundColor: placeableCubeHex(cube) ?? undefined };
}
