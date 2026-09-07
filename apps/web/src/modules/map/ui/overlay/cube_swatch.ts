// @pxl/web/modules/map/ui/overlay/cube_swatch.ts

import type { CSSProperties } from "react";
import { is_terrain_style_cube, placeable_cube_hex, type PlaceableCubeId } from "@pxl/shared/spec/cell/mod.ts";
import type { ChannelMap } from "@pxl/shared/spec/cell/type.ts";
import object_wire from "@pxl/shared/assets/cell/textures/objects/wire.ts";
import terrain_wire from "@pxl/shared/assets/cell/textures/terrain/wire.ts";

// object_wire/terrain_wire import real .png files (Vite-resolved URLs) —
// fine here (apps/web is Vite-built), but cell/mod.ts (packages/pxl/shared)
// is also loaded by plain `deno test`, which has no such transform and
// fails to import a .png as a module. This lookup stays in apps/web for
// that reason, mirroring objects/tex.ts's own get_object_texture (which
// only ever handles the flat-color/block half of it, for a THREE.Texture
// rather than a CSS url).
function placeable_cube_texture_url(id: PlaceableCubeId): string | null {
  if (is_terrain_style_cube(id)) {
    const [channel, local_key] = id.split(".") as [string, string];
    return (terrain_wire.top as ChannelMap)[channel]?.[local_key]?.url ?? null;
  }
  return (object_wire.top.block as Record<string, { url: string }>)[id]?.url ?? null;
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
