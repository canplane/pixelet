// @pxl/web/render/tex_array.ts

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

import { GROUND_LAYER_META, hex_to_rgb255 } from "./layer/terrain/shader_meta.ts";
import terrain_wire from "@pxl/shared/assets/cell/textures/terrain/wire.ts";
import { cube_hex, cube_ids, type CubeColorId } from "@pxl/shared/spec/cell/mod.ts";
import object_wire from "@pxl/shared/assets/cell/textures/objects/wire.ts";
import type { ChannelMap, ImageEntry } from "@pxl/shared/spec/cell/type.ts";

// Must match TILE_SIZE in cell/tools/download_texture_pack.ts.
const LAYER_TEX_SZ = 256;

function build_side_image_map(top: ChannelMap, overrides: Partial<ChannelMap>): ChannelMap {
  const out: ChannelMap = {};
  Object.keys(top).forEach((ch_key) => {
    const ch_out: Record<string, ImageEntry> = {};
    Object.keys(top[ch_key]).forEach((type_key) => {
      ch_out[type_key] = overrides[ch_key]?.[type_key] ?? top[ch_key][type_key];
    });
    out[ch_key] = ch_out;
  });
  return out;
}

type LayerMetaLike = { ch_key: string; type_key: string; layer_index: u32 };

/** Object colors, numbered starting right after every ground layer — see
 * this file's own doc comment. objects/adapter.ts reads this to resolve
 * VoxelMaterial.layer_idx the same way terrain/adapter.ts reads
 * GROUND_LAYER_INDEX_LOOKUP_FLAT.
 *
 * cube_ids (cell/mod.ts) is only ever the original 15 flat colors — the
 * terrain-styled catalog (2026-09-06's own cell/mod.ts, derived from
 * cube_types' own land/water/structure/transport channels) is a completely
 * separate numbering, deliberately never mixed into this one: those ids are
 * meant to look identical to an existing ground layer, not get a new one of
 * their own — objects/adapter.ts's own resolve_solid_material reads
 * GROUND_LAYER_INDEX_LOOKUP_FLAT directly for them instead of this table, so
 * cube_ids never needs filtering here. */
export const OBJECT_LAYER_IDX: Partial<Record<CubeColorId, u32>> = {};
const LAYER_META: LayerMetaLike[] = cube_ids.map((color_id, i) => {
  const layer_index = GROUND_LAYER_META.length + i;
  OBJECT_LAYER_IDX[color_id] = layer_index;
  return { ch_key: "block", type_key: color_id, layer_index };
});

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
    const layer_index = OBJECT_LAYER_IDX[color_id];
    if (layer_index === undefined) continue; // unreachable — every id in cube_ids gets an entry above.
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
 * Progressively replaces a layer array tex's flat-fill layers with real
 * decoded image pixels, once each mapped file finishes loading. No
 * PNG-decoding dependency: images load exactly the way any <img> on a
 * normal web page would, decoded by the browser itself, then read back via
 * a 2D canvas.
 */
function load_real_layer_images(
  tex: THREE.DataArrayTexture,
  layer_meta: readonly LayerMetaLike[],
  image_map: Record<string, Record<string, ImageEntry>>,
  sz: u32,
): void {
  const canvas = document.createElement("canvas");
  canvas.width = sz;
  canvas.height = sz;
  const ctx = canvas.getContext("2d", { willReadFrequently: true });
  if (!ctx) return;

  layer_meta.forEach((m) => {
    const entry = image_map[m.ch_key]?.[m.type_key];
    if (!entry) return;

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
      console.warn(`[tex_array] failed to load real tex for ${m.ch_key}.${m.type_key} — keeping flat-fill placeholder`);
    };
    img.src = entry.url;
  });
}

const placeholder_top = build_placeholder_data(LAYER_TEX_SZ);
const placeholder_side = placeholder_top.slice();

export const GROUND_LAYERS_TOP: THREE.DataArrayTexture = create_layer_array_tex(placeholder_top, LAYER_TEX_SZ, LAYER_TEX_SZ, LAYER_CNT);
export const GROUND_LAYERS_SIDE: THREE.DataArrayTexture = create_layer_array_tex(placeholder_side, LAYER_TEX_SZ, LAYER_TEX_SZ, LAYER_CNT);

load_real_layer_images(GROUND_LAYERS_TOP, COMBINED_META, { ...terrain_wire.top, ...object_wire.top }, LAYER_TEX_SZ);
load_real_layer_images(GROUND_LAYERS_SIDE, COMBINED_META, {
  ...build_side_image_map(terrain_wire.top, terrain_wire.side),
  ...build_side_image_map(object_wire.top, object_wire.side),
}, LAYER_TEX_SZ);
