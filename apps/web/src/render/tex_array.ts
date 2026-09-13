// @pxl/web/render/tex_array.ts

/**
 * Shared tex-pack layer arrays — real per-cell/per-object detail for
 * BOTH terrain ground types and placed-object colors, in one combined pair
 * of arrays (TOP/SIDE) so chunk_stage.tsx's shader only ever needs one
 * sampling path regardless of which WorldLayer a given cell came from.
 * Ground layers occupy indices [0, GROUND_LAYER_META.length) — same
 * numbering GROUND_LAYER_INDEX_LOOKUP_FLAT (shader_meta.ts) already uses,
 * unchanged — and object colors are appended right after, at
 * [GROUND_LAYER_META.length, total), the numbering shader_meta.ts's
 * OBJECT_LAYER_IDX gives them (object/adapter.ts's own consumer).
 *
 * Two arrays, TOP and SIDE, same layer numbering — a real ground type often
 * wants a different image on its side faces (grass-top/dirt-side) than its
 * top (TERRAIN_SIDE_IMAGE_MAP); placed objects have no such
 * distinction (a color block looks the same on every face), so their slice
 * of SIDE is just a second copy of their own TOP image.
 *
 * Placeholder-first, real-images-later: both arrays start as flat solid
 * fills so the very first rendered frame is already correct regardless of
 * network/decode timing, then load_real_layer_images asynchronously
 * upgrades individual layers in place as each mapped file finishes loading.
 */

import type { u32 } from "@xpute/core/kit/word.ts";

import * as THREE from "three";

import { GROUND_LAYER_META, hex_to_rgb255, OBJECT_LAYER_IDX } from "./layer/terrain/shader_meta.ts";
import { cube_hex, cube_ids } from "@pxl/shared/spec/cell/mod.ts";
import { type CubeFace, SHIPPED_SHEET, shipped_texture_tile } from "@pxl/shared/record/texture.ts";
import { asset_url } from "@/lib/env/route.ts";

// Must match TILE_SIZE in apps/api/src/bake/asset/tool/download_texture_pack.ts.
const LAYER_TEX_SZ = 256;

type LayerMetaLike = { ch_key: string; type_key: string; layer_index: u32 };

const LAYER_META: LayerMetaLike[] = cube_ids.map((color_id) => ({ ch_key: "block", type_key: color_id, layer_index: OBJECT_LAYER_IDX[color_id]! }));

const COMBINED_META: readonly LayerMetaLike[] = [...GROUND_LAYER_META, ...LAYER_META];
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
  tex.anisotropy = 4;

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
  for (const color_id of cube_ids) {
    const layer_index = OBJECT_LAYER_IDX[color_id]!;
    const [r, g, b] = hex_to_rgb255(cube_hex(color_id));
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
 * Replaces a layer array tex's flat-fill layers with the pack's real pixels
 * once the sheet arrives — one image holding every tile, one download, one
 * decode (record/texture.ts's SHIPPED_SHEET), read back tile by tile through
 * a 2D canvas. A side with no tile of its own wears the top's — the rule
 * the shader assumes when it samples the side array. Each layer is marked
 * for upload on its own and the texture updated once at the end: with sixty
 * separate images this re-uploaded the whole array per arrival.
 */
function load_sheet_layers(tex: THREE.DataArrayTexture, layer_meta: readonly LayerMetaLike[], face: CubeFace, sz: u32): void {
  const canvas = document.createElement("canvas");
  canvas.width = sz;
  canvas.height = sz;
  const ctx = canvas.getContext("2d", { willReadFrequently: true });
  if (!ctx) return;

  const img = new Image();
  img.crossOrigin = "anonymous";
  img.onload = () => {
    const cell = SHIPPED_SHEET.cell;
    for (const m of layer_meta) {
      const tile = shipped_texture_tile(m.ch_key, m.type_key, face) ?? shipped_texture_tile(m.ch_key, m.type_key, "top");
      if (!tile) continue;
      ctx.clearRect(0, 0, sz, sz);
      ctx.drawImage(img, tile.x, tile.y, cell, cell, 0, 0, sz, sz);
      const { data } = ctx.getImageData(0, 0, sz, sz);
      (tex.image.data as Uint8Array).set(data, m.layer_index * sz * sz * 4);
      tex.addLayerUpdate(m.layer_index);
    }
    tex.needsUpdate = true;
  };
  img.onerror = () => {
    console.warn(`[tex_array] failed to load the texture sheet — keeping flat-fill placeholders`);
  };
  img.src = asset_url(SHIPPED_SHEET.key);
}

const placeholder_top = build_placeholder_data(LAYER_TEX_SZ);
const placeholder_side = placeholder_top.slice();

export const GROUND_LAYERS_TOP: THREE.DataArrayTexture = create_layer_array_tex(placeholder_top, LAYER_TEX_SZ, LAYER_TEX_SZ, LAYER_CNT);
export const GROUND_LAYERS_SIDE: THREE.DataArrayTexture = create_layer_array_tex(placeholder_side, LAYER_TEX_SZ, LAYER_TEX_SZ, LAYER_CNT);

load_sheet_layers(GROUND_LAYERS_TOP, COMBINED_META, "top", LAYER_TEX_SZ);
load_sheet_layers(GROUND_LAYERS_SIDE, COMBINED_META, "side", LAYER_TEX_SZ);
