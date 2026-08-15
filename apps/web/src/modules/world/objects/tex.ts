// @pxl/web/modules/world/objects/tex.ts

import * as THREE from "three";
import { invalidate } from "@react-three/fiber";

import type { ObjectColorId } from "@pxl/shared/spec/cell/objects.ts";
import { OBJECT_IMAGE_MAP } from "@pxl/shared/spec/cell/theme/object_images.ts";

/**
 * Configured THREE.Texture from a URL — same create_* shape as
 * render/texture_array.ts's own create_layer_array_tex, just a plain image
 * load instead of a raw DataArrayTexture built from typed-array pixel data
 * (this one has a real source image to decode, so THREE.TextureLoader —
 * which texture_array.ts deliberately avoids, since ITS job is writing
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
  // visible right after flipping the low_spec ("텍스처팩") toggle with the
  // camera otherwise idle, since nothing else happens to trigger a frame.
  const tex = loader.load(url, () => invalidate());
  tex.colorSpace = THREE.SRGBColorSpace;
  // NearestFilter (not the default Linear/mipmap) — these are 16x16
  // pixel-art tiles; smoothing them blurs the wool weave into mush instead
  // of keeping the blocky look terrain's own splatmap/heightmap textures
  // (shader/tex.ts) already use.
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
// lifecycle terrain's own module-level texture_array.ts texture has,
// just lazily per-color instead of eagerly for every layer at
// module load, since objects.ts's catalog can grow far larger (200+
// planned) than terrain's own channel/type count and most colors may
// never actually get placed in a given session.
const cache = new Map<ObjectColorId, THREE.Texture>();

export function get_object_texture(id: ObjectColorId): THREE.Texture {
  let tex = cache.get(id);
  if (tex) return tex;
  tex = create_object_texture(OBJECT_IMAGE_MAP.block[id]);
  cache.set(id, tex);
  return tex;
}
