// @pxl/web/render/gpu/layers.ts

/**
 * The texture pack as two 2D array textures on the device — a top and a
 * side face per layer, one layer per ground type and cube colour, indexed
 * by the layer indices the mesher bakes into a vertex's taps
 * (render/gpu/layer_table.ts). Every layer starts as its flat colour, and
 * the pack's sheet replaces them tile by tile once it arrives.
 */

import type { u32 } from "@xpute/core/kit/word.ts";
import { cube_hex, cube_ids } from "@pxl/shared/spec/cell/mod.ts";
import { type CubeFace, SHIPPED_SHEET, shipped_texture_tile } from "@pxl/shared/record/texture.ts";

import { asset_url } from "@/lib/env/route.ts";
import { GROUND_LAYER_META, hex_to_rgb255, OBJECT_LAYER_IDX } from "@/render/gpu/layer_table.ts";
import { mip_level_count, type MipBuilder } from "./mips.ts";

/** Must match TILE_SIZE in apps/api/src/bake/asset/tool/download_texture_pack.ts. */
export const LAYER_TEX_SZ = 256;

type LayerMeta = { ch_key: string; type_key: string; layer_index: u32; hex: string };

const LAYERS: readonly LayerMeta[] = [
  ...GROUND_LAYER_META.map((m) => ({ ch_key: m.ch_key, type_key: m.type_key, layer_index: m.layer_index, hex: m.hex_color })),
  ...cube_ids.map((color_id) => ({ ch_key: "block", type_key: color_id, layer_index: OBJECT_LAYER_IDX[color_id]!, hex: cube_hex(color_id) })),
];
export const LAYER_CNT: u32 = LAYERS.length;

export type LayerTextures = { top: GPUTexture; side: GPUTexture; sampler: GPUSampler; bytes: u32 };

/** Both arrays, flat-filled now and the sheet's tiles as it arrives, each
 * with its mip chain: a tile a cell minifies to a few texels at the far
 * tiers, and without the chain every camera movement resamples a different
 * texel and the ground crawls. A chain is a third again of the array. */
export function create_layer_textures(device: GPUDevice, mips: MipBuilder): LayerTextures {
  const levels = mip_level_count(LAYER_TEX_SZ, LAYER_TEX_SZ);
  const make = (label: string) =>
    device.createTexture({
      label,
      size: { width: LAYER_TEX_SZ, height: LAYER_TEX_SZ, depthOrArrayLayers: LAYER_CNT },
      format: "rgba8unorm-srgb",
      mipLevelCount: levels,
      usage: GPUTextureUsage.TEXTURE_BINDING | GPUTextureUsage.COPY_DST | GPUTextureUsage.RENDER_ATTACHMENT,
    });
  const top = make("layers/top");
  const side = make("layers/side");
  const fill = new Uint8Array(LAYER_TEX_SZ * LAYER_TEX_SZ * 4);
  for (const m of LAYERS) {
    const [r, g, b] = hex_to_rgb255(m.hex);
    for (let i = 0; i < LAYER_TEX_SZ * LAYER_TEX_SZ; i++) {
      fill[i * 4] = r;
      fill[i * 4 + 1] = g;
      fill[i * 4 + 2] = b;
      fill[i * 4 + 3] = 255;
    }
    for (const tex of [top, side]) {
      device.queue.writeTexture({ texture: tex, origin: [0, 0, m.layer_index] }, fill, { bytesPerRow: LAYER_TEX_SZ * 4 }, [LAYER_TEX_SZ, LAYER_TEX_SZ, 1]);
    }
  }
  mips.build(top, 0, LAYER_CNT);
  mips.build(side, 0, LAYER_CNT);
  load_sheet(device, top, "top", mips);
  load_sheet(device, side, "side", mips);
  const sampler = device.createSampler({ addressModeU: "repeat", addressModeV: "repeat", magFilter: "linear", minFilter: "linear", mipmapFilter: "linear" });
  // A chain is the sum of the levels' texels: under four thirds of the first.
  let bytes = 0;
  for (let level = 0; level < levels; level++) bytes += (LAYER_TEX_SZ >> level) ** 2 * 4 * LAYER_CNT * 2;
  return { top, side, sampler, bytes };
}

/** The pack's sheet read tile by tile into the array; a side with no tile
 * of its own wears the top's, the rule the shader assumes. */
function load_sheet(device: GPUDevice, tex: GPUTexture, face: CubeFace, mips: MipBuilder): void {
  const img = new Image();
  img.crossOrigin = "anonymous";
  img.onload = () => {
    const canvas = document.createElement("canvas");
    canvas.width = LAYER_TEX_SZ;
    canvas.height = LAYER_TEX_SZ;
    const ctx = canvas.getContext("2d");
    if (!ctx) return;
    const cell = SHIPPED_SHEET.cell;
    for (const m of LAYERS) {
      const tile = shipped_texture_tile(m.ch_key, m.type_key, face) ?? shipped_texture_tile(m.ch_key, m.type_key, "top");
      if (!tile) continue;
      ctx.clearRect(0, 0, LAYER_TEX_SZ, LAYER_TEX_SZ);
      ctx.drawImage(img, tile.x, tile.y, cell, cell, 0, 0, LAYER_TEX_SZ, LAYER_TEX_SZ);
      device.queue.copyExternalImageToTexture({ source: canvas }, { texture: tex, origin: [0, 0, m.layer_index] }, [LAYER_TEX_SZ, LAYER_TEX_SZ, 1]);
    }
    mips.build(tex, 0, LAYER_CNT);
  };
  img.onerror = () => console.warn(`[gpu/layers] failed to load the texture sheet — keeping flat-fill layers`);
  img.src = asset_url(SHIPPED_SHEET.key);
}
