// @pxl/web/modules/world/objects/tex.ts

import * as THREE from "three";
import { invalidate } from "@react-three/fiber";

import { is_terrain_style_cube, type PlaceableCubeId } from "@pxl/shared/spec/cell/mod.ts";
import object_wire from "@pxl/shared/assets/cell/textures/objects/wire.ts";
import terrain_wire from "@pxl/shared/assets/cell/textures/terrain/wire.ts";

/**
 * Configured THREE.Texture from a URL — same create_* shape as
 * render/tex_array.ts's own create_layer_array_tex, just a plain image
 * load instead of a raw DataArrayTexture built from typed-array pixel data
 * (this one has a real source image to decode, so THREE.TextureLoader —
 * which tex_array.ts deliberately avoids, since ITS job is writing
 * decoded pixels into an existing DataArrayTexture's own buffer, something
 * TextureLoader has no hook for — is the correct, unreinvented tool here).
 */
const loader = new THREE.TextureLoader();

function create_object_texture(url: string): THREE.Texture {
  // scene.tsx runs frameloop="demand" — it only redraws when something
  // calls invalidate(). The image itself loads async well after this
  // function returns, so without this the canvas can be sitting on a
  // frame rendered before the image arrived (a blank/white texture, since
  // that's what an unresolved THREE.Texture samples as) and never redraw
  // it, even though the texture object itself updated correctly — most
  // visible right after flipping the texture_pack setting with the
  // camera otherwise idle, since nothing else happens to trigger a frame.
  const tex = loader.load(url, () => invalidate());
  tex.colorSpace = THREE.SRGBColorSpace;
  // NearestFilter (not the default Linear/mipmap) — these are pixel-art
  // tiles; smoothing them blurs their painted surface detail into mush
  // instead of keeping the blocky look terrain's own splatmap/heightmap
  // textures (shader/tex.ts) already use.
  tex.magFilter = THREE.NearestFilter;
  tex.minFilter = THREE.NearestFilter;
  tex.generateMipmaps = false;
  tex.anisotropy = 1;
  // RepeatWrapping (not the default ClampToEdgeWrapping) — an octree-
  // merged multi-cell block scales its own geometry UVs past 1 to tile
  // this texture once per real cell (block.tsx), which needs
  // wrapping to actually repeat rather than smear the edge texel.
  tex.wrapS = THREE.RepeatWrapping;
  tex.wrapT = THREE.RepeatWrapping;
  return tex;
}

// One real THREE.Texture per color, built once and reused by every placed
// cube of that color — same "build once, share across every consumer"
// lifecycle terrain's own module-level tex_array.ts texture has,
// just lazily per-color instead of eagerly for every layer at
// module load, since objects.ts's catalog can grow far larger (200+
// planned) than terrain's own channel/type count and most colors may
// never actually get placed in a given session.
const cache = new Map<string, THREE.Texture>();

/** Which of a box's images this is. The split is the renderer's own:
 * chunk_material.ts calls a face lateral at `abs(normal.z) < 0.5`, so a
 * bottom face wears the top image just as the top face does. */
export type CubeFace = "top" | "side";

/**
 * The real image a cube of this kind renders with on this face, flat colour
 * or terrain-styled alike. A terrain-styled pick is `"<channel>.<KEY>"` and
 * that is exactly how both wires are keyed, so the two catalogs differ only
 * in which one holds the entry.
 *
 * This used to return null for every terrain-styled id, on the grounds that
 * object_wire has no entry for one — true, and terrain_wire does. The ghost
 * preview then fell back to a flat colour swatch, which is what made the
 * eraser tint a terrain-styled cube with the generic action colour instead
 * of showing the thing it was about to remove.
 */
export function get_object_texture(id: PlaceableCubeId, face: CubeFace = "top"): THREE.Texture | null {
  const wire = wire_entry(id, face);
  if (!wire) return null;
  // Keyed by URL, not by id+face: a side that falls back to its own top
  // image has to resolve to the one THREE.Texture that image already has,
  // or a ghost would upload the same picture twice and draw its sides from
  // a second copy.
  let tex = cache.get(wire.url) ?? null;
  if (tex) return tex;
  tex = create_object_texture(wire.url);
  cache.set(wire.url, tex);
  return tex;
}

type WireEntry = { url: string } | undefined;

/** Side falls back to top when a kind declares no side of its own — the
 * same rule render/tex_array.ts's build_side_image_map bakes into the layer
 * array the real chunks sample. Only the land channel overrides it today;
 * object blocks never do. */
type FaceMap = Record<string, Record<string, WireEntry> | undefined>;

function wire_entry(id: PlaceableCubeId, face: CubeFace): WireEntry {
  const wire = (is_terrain_style_cube(id) ? terrain_wire : object_wire) as Record<CubeFace, FaceMap>;
  const [channel, key] = is_terrain_style_cube(id) ? id.split(".") : ["block", id];
  return wire[face][channel]?.[key] ?? wire.top[channel]?.[key];
}
