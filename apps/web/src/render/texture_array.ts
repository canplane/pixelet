// @pxl/web/render/texture_array.ts

/**
 * Shared tex-pack layer arrays — real per-cell/per-object detail for
 * BOTH terrain ground types and placed-object colors, in one combined pair
 * of arrays (TOP/SIDE) so chunk_stage.tsx's shader only ever needs one
 * sampling path regardless of which WorldLayer a given cell came from.
 * Ground layers occupy indices [0, GROUND_LAYER_META.length) — same
 * numbering GROUND_LAYER_INDEX_LOOKUP_FLAT (shader_meta.ts) already uses,
 * unchanged — and object colors are appended right after, at
 * [GROUND_LAYER_META.length, total), looked up via OBJECT_LAYER_IDX
 * below (objects/adapter.ts's own consumer).
 *
 * Two arrays, TOP and SIDE, same layer numbering — a real ground type often
 * wants a different image on its side faces (grass-top/dirt-side) than its
 * top (GROUND_LAYER_SIDE_IMAGE_MAP); placed objects have no such
 * distinction (a wool block looks the same on every face), so their slice
 * of SIDE is just a second copy of their own TOP image.
 *
 * Placeholder-first, real-images-later: both arrays start as flat solid
 * fills so the very first rendered frame is already correct regardless of
 * network/decode timing, then load_real_layer_images asynchronously
 * upgrades individual layers in place as each mapped file finishes loading.
 */

import type { u32 } from "@xpute/core/kit/word.ts";

import * as THREE from "three";

import { GROUND_LAYER_META, hex_to_rgb255 } from "./layer/terrain/shader_meta.ts";
import { GROUND_LAYER_IMAGE_MAP, GROUND_LAYER_SIDE_IMAGE_MAP, type GroundLayerImageEntry } from "@pxl/shared/spec/cell/theme/terrain_images.ts";
import { object_color_hex, object_color_ids, type ObjectColorId } from "@pxl/shared/spec/cell/objects.ts";
import { OBJECT_IMAGE_MAP } from "@pxl/shared/spec/cell/theme/object_images.ts";

const LAYER_TEX_SZ = 16;

type LayerMetaLike = { ch_key: string; type_key: string; layer_index: u32 };

/** Object colors, numbered starting right after every ground layer — see
 * this file's own doc comment. objects/adapter.ts reads this to resolve
 * VoxelMaterial.layer_idx the same way terrain/adapter.ts reads
 * GROUND_LAYER_INDEX_LOOKUP_FLAT. */
export const OBJECT_LAYER_IDX: Partial<Record<ObjectColorId, u32>> = {};
const OBJECT_LAYER_META: LayerMetaLike[] = object_color_ids.map((color_id, i) => {
  const layer_index = GROUND_LAYER_META.length + i;
  OBJECT_LAYER_IDX[color_id] = layer_index;
  return { ch_key: "object", type_key: color_id, layer_index };
});

const COMBINED_META: readonly LayerMetaLike[] = [...GROUND_LAYER_META, ...OBJECT_LAYER_META];
const LAYER_CNT: u32 = COMBINED_META.length;

function create_layer_array_tex(data: Uint8Array, width: u32, height: u32, depth: u32): THREE.DataArrayTexture {
  const tex = new THREE.DataArrayTexture(data, width, height, depth);
  tex.format = THREE.RGBAFormat;
  tex.type = THREE.UnsignedByteType;

  // decode_ground_rgba/hex_to_rgb255 both hand back raw sRGB-encoded hex
  // bytes — same convention create_ground_color_tex (the old,
  // now-archived tex.ts) already tagged explicitly.
  tex.colorSpace = THREE.SRGBColorSpace;

  tex.magFilter = THREE.LinearFilter;
  tex.minFilter = THREE.LinearMipmapLinearFilter;
  tex.generateMipmaps = true;
  // deliberately not anisotropic — see scene.tsx's tilt-shift/miniature
  // discussion: aniso sharpens exactly the oblique-angle detail the
  // "diorama" look wants softened.
  tex.anisotropy = 1;

  tex.wrapS = THREE.RepeatWrapping;
  tex.wrapT = THREE.RepeatWrapping;
  tex.needsUpdate = true;
  return tex;
}

/** Flat solid-fill placeholder for every layer (ground + object), same
 * hex color the real image will eventually be replaced by — visually
 * close enough to be a real "loading" state, not a jarring one. */
function build_placeholder_data(sz: u32): Uint8Array {
  const data = new Uint8Array(sz * sz * LAYER_CNT * 4);
  for (const m of GROUND_LAYER_META) {
    const [r, g, b] = hex_to_rgb255(m.hex_color);
    fill_layer(data, m.layer_index, sz, r, g, b);
  }
  for (const color_id of object_color_ids) {
    const layer_index = OBJECT_LAYER_IDX[color_id];
    if (layer_index === undefined) continue; // unreachable — every id in object_color_ids gets an entry above.
    const [r, g, b] = hex_to_rgb255(object_color_hex(color_id));
    fill_layer(data, layer_index, sz, r, g, b);
  }
  return data;
}

function fill_layer(data: Uint8Array, layer_index: u32, sz: u32, r: u32, g: u32, b: u32): void {
  const layer_off = layer_index * sz * sz * 4;
  for (let i = 0; i < sz * sz; i++) {
    const o = layer_off + i * 4;
    data[o] = r;
    data[o + 1] = g;
    data[o + 2] = b;
    data[o + 3] = 255;
  }
}

/**
 * Progressively replaces a layer array tex's flat-fill layers with real
 * decoded image pixels, once each mapped file finishes loading. No
 * PNG-decoding dependency: images load exactly the way any <img> on a
 * normal web page would, decoded by the browser itself, then read back via
 * a 2D canvas.
 */
function load_real_layer_images(
  tex: THREE.DataArrayTexture,
  layer_meta: readonly LayerMetaLike[],
  image_map: Record<string, Record<string, GroundLayerImageEntry | string>>,
  sz: u32,
): void {
  const canvas = document.createElement("canvas");
  canvas.width = sz;
  canvas.height = sz;
  const ctx = canvas.getContext("2d", { willReadFrequently: true });
  if (!ctx) return;

  layer_meta.forEach((m) => {
    const raw = image_map[m.ch_key]?.[m.type_key];
    if (!raw) return;
    const entry: GroundLayerImageEntry = typeof raw === "string" ? { url: raw } : raw;

    const img = new Image();
    img.crossOrigin = "anonymous";
    img.onload = () => {
      ctx.clearRect(0, 0, sz, sz);
      // Only the first frame_height-tall slice for animated sprite sheets
      // (frames stacked vertically) — this pipeline has no concept of
      // animation, so "first frame" is the only sensible still image.
      const src_h = entry.frame_height ?? img.naturalHeight;
      ctx.drawImage(img, 0, 0, img.naturalWidth, src_h, 0, 0, sz, sz);
      const { data } = ctx.getImageData(0, 0, sz, sz);

      const layer_off = m.layer_index * sz * sz * 4;
      (tex.image.data as Uint8Array).set(data, layer_off);
      tex.needsUpdate = true;
    };
    img.onerror = () => {
      console.warn(`[texture_array] failed to load real tex for ${m.ch_key}.${m.type_key} — keeping flat-fill placeholder`);
    };
    img.src = entry.url;
  });
}

const placeholder_top = build_placeholder_data(LAYER_TEX_SZ);
const placeholder_side = placeholder_top.slice();

export const GROUND_LAYERS_TOP: THREE.DataArrayTexture = create_layer_array_tex(placeholder_top, LAYER_TEX_SZ, LAYER_TEX_SZ, LAYER_CNT);
export const GROUND_LAYERS_SIDE: THREE.DataArrayTexture = create_layer_array_tex(placeholder_side, LAYER_TEX_SZ, LAYER_TEX_SZ, LAYER_CNT);

load_real_layer_images(GROUND_LAYERS_TOP, COMBINED_META, { ...GROUND_LAYER_IMAGE_MAP, object: OBJECT_IMAGE_MAP.block }, LAYER_TEX_SZ);
// objects have no distinct side image — build_side_image_map's own
// top-fallback pattern, just done here directly instead of via that helper.
load_real_layer_images(GROUND_LAYERS_SIDE, COMBINED_META, { ...GROUND_LAYER_SIDE_IMAGE_MAP, object: OBJECT_IMAGE_MAP.block }, LAYER_TEX_SZ);
