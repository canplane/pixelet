// @pxl/web/modules/world/terrain/shader/tex.ts

import * as THREE from "three";

import type { u32 } from "@xpute/core/kit/word.ts";
import type { I16Array, U8Array } from "@xpute/core/kit/array.ts";

export function create_splatmap(ch: U8Array, row_stride: u32): THREE.DataTexture {
  const tex = new THREE.DataTexture(ch, row_stride, row_stride, THREE.RedFormat, THREE.UnsignedByteType);
  tex.internalFormat = "R8";

  tex.magFilter = THREE.NearestFilter;
  tex.minFilter = THREE.NearestFilter;
  tex.generateMipmaps = false;
  tex.anisotropy = 1;

  tex.wrapS = THREE.ClampToEdgeWrapping;
  tex.wrapT = THREE.ClampToEdgeWrapping;
  tex.needsUpdate = true;
  return tex;
}

/**
 * RGBA texture built from decode_ground_rgba's already-color-resolved
 * output — unlike create_splatmap's raw category-code textures (Nearest,
 * no mips: a category id can't be filtered), every texel here is already a
 * real color, so real hardware mip generation is valid. Sampled via
 * textureLod (see shader.ts's create_vert) so u_lod_step selects an exact
 * mip level instead of relying on screen-space-derivative auto selection,
 * which core_uv's per-instance-constant nature would defeat anyway.
 */
export function create_ground_color_texture(rgba: Uint8Array, row_stride: u32): THREE.DataTexture {
  const tex = new THREE.DataTexture(rgba, row_stride, row_stride, THREE.RGBAFormat, THREE.UnsignedByteType);

  // decode_ground_rgba stores the raw hex bytes verbatim (sRGB-encoded,
  // gamma-curved perceptual color — the same hex strings terrain_colors
  // already used) — same convention THREE.Color(hex_color) already applies
  // to u_color_ch_* (structure) via three's ColorManagement, which
  // sRGB-decodes automatically on construction. Without this flag, this
  // texture's raw encoded bytes would reach the shader un-decoded: mixed
  // with the rest of the pipeline's linear-space values and gamma-corrected
  // *again* at output, reading as an overall too-bright/washed-out look —
  // worse at coarse LOD, since gl.generateMipmap() on a plain (non-sRGB)
  // format box-averages the encoded bytes directly, which is itself a
  // classic source of mips reading brighter than the color they're
  // supposed to represent. SRGBColorSpace makes both the sample *and* the
  // mip generation gamma-aware in hardware.
  tex.colorSpace = THREE.SRGBColorSpace;

  tex.magFilter = THREE.LinearFilter;
  tex.minFilter = THREE.LinearMipmapLinearFilter;
  tex.generateMipmaps = true;
  tex.anisotropy = 1;

  tex.wrapS = THREE.ClampToEdgeWrapping;
  tex.wrapT = THREE.ClampToEdgeWrapping;
  tex.needsUpdate = true;
  return tex;
}

/**
 * Shared ground-layer texture array — one layer per (channel, type) pair
 * (see shader_meta.ts's get_layer_meta/build_layer_texture_data), built
 * ONCE at module load (mat.ts) and reused by every chunk's material, unlike
 * create_ground_color_texture's old per-chunk RGBA bake. Each layer is real
 * image data (solid-fill placeholder today, a real tileable image later —
 * see build_layer_texture_data's own comment), so hardware mip generation
 * just works, same reasoning as create_ground_color_texture's own comment.
 *
 * RepeatWrapping, not ClampToEdge: the old shared-atlas texture needed
 * clamping to avoid bleeding between adjacent blocks packed into the same
 * image. Each layer here is independent (its own slice of the array, no
 * neighbors to bleed into), and the texture-pack sampling path (shader.ts's
 * create_frag) deliberately tiles across a cell's own face — clamping would
 * just stretch a single edge texel across the whole cell instead.
 */
export function create_ground_layer_texture(data: Uint8Array, width: u32, height: u32, depth: u32): THREE.DataArrayTexture {
  const tex = new THREE.DataArrayTexture(data, width, height, depth);
  tex.format = THREE.RGBAFormat;
  tex.type = THREE.UnsignedByteType;

  // see create_ground_color_texture's own comment — same sRGB-encoded hex
  // source, same reasoning for tagging colorSpace explicitly.
  tex.colorSpace = THREE.SRGBColorSpace;

  tex.magFilter = THREE.LinearFilter;
  tex.minFilter = THREE.LinearMipmapLinearFilter;
  tex.generateMipmaps = true;
  // deliberately not anisotropic — see the miniature/tilt-shift discussion:
  // aniso sharpens exactly the oblique-angle detail the tilt-shift blur is
  // trying to soften, fighting the diorama illusion instead of helping it.
  tex.anisotropy = 1;

  tex.wrapS = THREE.RepeatWrapping;
  tex.wrapT = THREE.RepeatWrapping;
  tex.needsUpdate = true;
  return tex;
}

/** ch_key -> type_key -> which real image that type samples in
 * texture-pack mode — see ground_layer_images.ts's own doc comment. */
export type GroundLayerImageMap = Record<string, Record<string, { url: string; frame_height?: number }>>;

type LayerMetaLike = { ch_key: string; type_key: string; layer_index: number };

/**
 * Progressively replaces a ground_layers array texture's flat-fill layers
 * with real decoded image pixels, once each mapped file finishes loading
 * (see ground_layer_images.ts for the actual ch/type -> image mapping).
 * create_ground_layer_texture already built a fully-usable (flat-fill)
 * texture before this ever runs, so the very first rendered frame is
 * always correct regardless of network/decode timing — this only
 * asynchronously *upgrades* individual layers as their real images finish
 * loading. Deliberately no PNG-decoding dependency anywhere in this
 * pipeline: images are loaded exactly the way any <img> on a normal web
 * page would be, decoded by the browser itself, then read back via a 2D
 * canvas — the same "load once at startup, decode natively" pattern
 * three.js's own TextureLoader uses internally, just done by hand here
 * since DataArrayTexture (unlike a plain Texture) has no built-in loader.
 *
 * A type with no entry in `image_map` simply keeps whatever
 * build_layer_texture_data already gave it — this is purely additive.
 */
export function load_real_ground_layer_images(
  tex: THREE.DataArrayTexture,
  layer_meta: readonly LayerMetaLike[],
  image_map: GroundLayerImageMap,
  size: u32,
): void {
  const canvas = document.createElement("canvas");
  canvas.width = size;
  canvas.height = size;
  const ctx = canvas.getContext("2d", { willReadFrequently: true });
  if (!ctx) return;

  layer_meta.forEach((m) => {
    const entry = image_map[m.ch_key]?.[m.type_key];
    if (!entry) return;

    const img = new Image();
    img.crossOrigin = "anonymous";
    img.onload = () => {
      ctx.clearRect(0, 0, size, size);
      // Only the first frame_height-tall slice for animated sprite sheets
      // (frames stacked vertically, e.g. a 16x256 sheet = 16 frames of
      // 16x16) — this pipeline has no concept of animation, so "first
      // frame" is the only sensible still image to show. Static
      // single-frame sources (the common case) omit frame_height and use
      // their own natural size instead. Either way the source draws
      // scaled to this layer's own (size x size) slot, regardless of the
      // source image's native resolution.
      const src_h = entry.frame_height ?? img.naturalHeight;
      ctx.drawImage(img, 0, 0, img.naturalWidth, src_h, 0, 0, size, size);
      const { data } = ctx.getImageData(0, 0, size, size);

      const layer_offset = m.layer_index * size * size * 4;
      (tex.image.data as Uint8Array).set(data, layer_offset);
      tex.needsUpdate = true;
    };
    img.onerror = () => {
      console.warn(`[ground_layers] failed to load real texture for ${m.ch_key}.${m.type_key} — keeping flat-fill placeholder`);
    };
    img.src = entry.url;
  });
}

export function create_heightmap(ch: I16Array, row_stride: u32): THREE.DataTexture {
  const tex = new THREE.DataTexture(ch, row_stride, row_stride, THREE.RedIntegerFormat, THREE.ShortType);
  tex.internalFormat = "R16I";
  tex.magFilter = THREE.NearestFilter;
  tex.minFilter = THREE.NearestFilter;
  tex.generateMipmaps = false;
  tex.wrapS = THREE.ClampToEdgeWrapping;
  tex.wrapT = THREE.ClampToEdgeWrapping;
  tex.needsUpdate = true;
  return tex;
}
