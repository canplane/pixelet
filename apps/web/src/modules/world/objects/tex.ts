// @pxl/web/modules/world/objects/tex.ts

import * as THREE from "three";
import { invalidate } from "@react-three/fiber";

import type { PlaceableCubeId } from "@pxl/shared/spec/cell/mod.ts";
import object_wire from "@pxl/shared/assets/cell/textures/objects/wire.ts";

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
  // visible right after flipping the low_spec ("Texture Pack") toggle with the
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
const cache = new Map<PlaceableCubeId, THREE.Texture>();

/**
 * `null` for a terrain-styled id (cell/mod.ts's own TERRAIN_STYLE_SOURCE,
 * 2026-09-06) — those have no entry of their own in object_wire (they're
 * not a separate texture-array layer at all, see tex_array.ts's own
 * TERRAIN_STYLE_SOURCE-skip), so there's no per-color wire image to load
 * here. hover_cursor.tsx's own ghost preview already degrades gracefully
 * to a flat color swatch when this returns null; a real placed cube of
 * this kind still renders with its own real terrain texture regardless,
 * through chunk_stage.tsx's own shader — this only affects the
 * pre-placement preview's fidelity, not the real render.
 */
export function get_object_texture(id: PlaceableCubeId): THREE.Texture | null {
  let tex = cache.get(id) ?? null;
  if (tex) return tex;
  const wire = object_wire.top.block[id as keyof typeof object_wire.top.block] as { url: string } | undefined;
  if (!wire) return null;
  tex = create_object_texture(wire.url);
  cache.set(id, tex);
  return tex;
}
