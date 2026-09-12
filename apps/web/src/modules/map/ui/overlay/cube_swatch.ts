// @pxl/web/modules/map/ui/overlay/cube_swatch.ts

import type { CSSProperties } from "react";
import { placeable_cube_hex, type PlaceableCubeId } from "@pxl/shared/spec/cell/mod.ts";
import { asset_url } from "@/lib/env/routes.ts";
import { texture_key } from "@/modules/world/objects/tex.ts";

function placeable_cube_texture_url(id: PlaceableCubeId): string | null {
  const key = texture_key(id, "top");
  return key ? asset_url(key) : null;
}

/** Style for a cube swatch — the real texture-pack top image when one
 * exists, falling back to a flat color swatch otherwise (the same fallback
 * hover_cursor.tsx's own ghost preview already relies on for
 * objects/tex.ts's own get_object_texture). Shared by drawer_modal.tsx and
 * mode_bar.tsx so their swatches never drift apart. */
export function cube_swatch_style(id: PlaceableCubeId): CSSProperties {
  const texture_url = placeable_cube_texture_url(id);
  if (texture_url) {
    return { backgroundImage: `url(${texture_url})`, backgroundSize: "cover", imageRendering: "pixelated" };
  }
  return { backgroundColor: placeable_cube_hex(id) ?? undefined };
}
