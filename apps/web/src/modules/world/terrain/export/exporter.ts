// @pxl/web/modules/world/terrain/export/exporter.ts

/**
 * @file tile-exporter.ts
 * 엔진 내부의 채널 데이터(Uint8Array)를 시각적 이미지(PNG)로 추출합니다.
 * 베이킹 엔진(Baker)에서 생성된 복셀 데이터가 실제 지형과 일치하는지 검증하는 용도입니다.
 *
 * [Refactor Note]
 * 1. VOXEL_TYPES/COLORS를 순회하여 색상 맵을 자동 생성합니다.
 * 2. GRID_CHANNEL_KEYS를 순회하여 모든 채널을 동적으로 합성(Channeling)합니다.
 * 3. [Update] Major(Channel)/Minor(Type) 위계 구조에 맞춰 채널별 독립 색상 매핑을 지원합니다.
 * 4. [Separation] 데이터 생성과 파일 저장(IO) 로직을 분리하여 범용성을 높였습니다.
 */

import type { u32, u8 } from "@xpute/spec/lib/kit/word.ts";
import type { U8Array } from "@xpute/spec/lib/kit/array.ts";

import { type Ground, GROUND_CH_KEYS, type GroundChannelKey } from "@pxl/bake/terrain/format.ts";
import { PAGE_CELL_SPAN } from "@pxl/bake/terrain/constants.ts";
import { cell_colors, cell_types } from "@pxl/spec/cell/mod.ts";

import { KEYS } from "@pxl/render/terrain/shader_meta.ts";

type RGB = [u8, u8, u8];
type ColorMap = Record<GroundChannelKey, Record<u8, RGB>>;

/** HEX(#RRGGBB 또는 RRGGBB)를 [r, g, b]로 변환 */
const hex_to_rgb = (hex: string): RGB => {
  // 'transparent' 등의 예외 처리 (검은색 반환)
  if (!hex || !hex.startsWith("#")) return [0, 0, 0];

  const clean_hex = hex.slice(1);
  const r: u8 = parseInt(clean_hex.slice(0, 2), 16);
  const g: u8 = parseInt(clean_hex.slice(2, 4), 16);
  const b: u8 = parseInt(clean_hex.slice(4, 6), 16);
  return [r, g, b];
};

const get_color_map = (): ColorMap => {
  const map = {
    land: {},
    water: {},
    structure: {},
    transport: {},
  } as ColorMap;

  for (const ch_key of GROUND_CH_KEYS) {
    const ch_types = cell_types.terrain[ch_key];
    const ch_colors = cell_colors.terrain[ch_key];

    for (const type_key of KEYS(ch_types)) {
      const type_id = ch_types[type_key];
      const hex_color = ch_colors[type_id];
      if (hex_color) {
        map[ch_key][type_id] = hex_to_rgb(hex_color);
      }
    }
  }

  return map;
};

/**
 * 캔버스 좌표(x, y)에 대응하는 월드 데이터(South-to-North)의 인덱스를 반환합니다.
 *
 * 캔버스의 y는 위(0)에서 아래(PAGE_CELL_SPAN - 1)로 가지만,
 * 월드 데이터의 gy는 아래(0)에서 위(PAGE_CELL_SPAN - 1)로 가므로 여기서 변환
 */
const IDX = (gx: u32, gy: u32): u32 => (PAGE_CELL_SPAN - 1 - gy) * PAGE_CELL_SPAN + gx;

/**
 * [Internal Helper] 캔버스 생성 및 픽셀 주입 공통 로직
 */
async function draw_to_blob(
  func: (idx: u32, color_map: ColorMap) => RGB,
): Promise<Blob | null> {
  if (typeof document === "undefined") return null;

  const canvas = document.createElement("canvas");
  canvas.width = PAGE_CELL_SPAN;
  canvas.height = PAGE_CELL_SPAN;
  const ctx = canvas.getContext("2d");
  if (!ctx) return null;

  const image = ctx.createImageData(PAGE_CELL_SPAN, PAGE_CELL_SPAN);
  const data = image.data;

  /**
   * 🎯 [Optimization: Sequential Write]
   * 캔버스 좌표(cx, cy)를 기준으로 루프를 돌며 ImageData를 순차적으로 채웁니다.
   * 이렇게 하면 data[i]에 순서대로 접근하므로 메모리 대역폭을 최대로 활용합니다.
   */
  let canvas_idx: u32 = 0;
  for (let cy: u32 = 0; cy < PAGE_CELL_SPAN; cy++) {
    // Top to Bottom
    for (let cx: u32 = 0; cx < PAGE_CELL_SPAN; cx++) {
      // Left to Right

      // 1. 현재 캔버스 위치(cy, cx)에 해당하는 데이터 인덱스 계산
      const idx: u32 = IDX(cx, cy);

      // 2. 데이터 추출
      const rgb: RGB = func(idx, get_color_map());

      // 3. 순차적 쓰기 (Zero-Jump)
      data[canvas_idx++] = rgb[0];
      data[canvas_idx++] = rgb[1];
      data[canvas_idx++] = rgb[2];
      data[canvas_idx++] = 255;
    }
  }

  ctx.putImageData(image, 0, 0);
  return await new Promise((resolve) => canvas.toBlob(resolve, "image/png"));
}

/**
 * [exportGridImage]
 * 단일 채널(예: Water, Transport) 그리드를 시각화하여 Blob으로 반환합니다.
 * 🎯 [Update] 채널별 ID 스코핑을 위해 ch_key를 참조하도록 수정되었습니다.
 */
export function export_ground_ch_image(
  ch: U8Array,
  ch_key: GroundChannelKey,
): Promise<Blob | null> {
  return draw_to_blob((idx, color_map) => {
    const cell_val: u8 = ch[idx];
    const ch_colors = color_map[ch_key] || {};
    const FALLBACK_RGB = color_map.land[cell_types.terrain.land.BARE];

    if (cell_val === cell_types.common.AIR) return [0, 0, 0];
    if (cell_val === cell_types.common.UNKNOWN) return [0, 0, 0];
    return ch_colors[cell_val] || FALLBACK_RGB;
  });
}

/**
 * [exportMergedGridImage]
 * 🚀 [Channel Automation] TILE_CHANNEL_KEYS에 정의된 모든 채널을 합성하여 Blob으로 반환합니다.
 * 쉐이더의 Painter's Algorithm을 그대로 JS로 구현하여 최종 결과물을 시뮬레이션합니다.
 */
export function export_ground_image(
  ground: Ground,
): Promise<Blob | null> {
  return draw_to_blob((i, colorMap) => {
    /**
     * [Painter's Algorithm 정교화]
     * 0(AIR)으로 시작하여 TILE_CHANNEL_KEYS 순서대로 유효한 값을 덮어씌웁니다.
     */
    let final_rgb: RGB | null = null;
    const FALLBACK_RGB = colorMap.land[cell_types.terrain.land.BARE];

    for (const key of GROUND_CH_KEYS) {
      const val = ground[key][i];
      if (val === cell_types.common.AIR) continue;
      if (val === cell_types.common.UNKNOWN) continue;
      const ch_color = colorMap[key]?.[val];
      if (ch_color) final_rgb = ch_color;
    }

    /**
     * 최종 색상 결정
     * 모든 채널을 검사했는데도 데이터가 없다면(AIR/UNKNOWN),
     * 시각적 결손 방지를 위해 BASE_LAND 색상을 적용합니다.
     */
    return final_rgb || FALLBACK_RGB;
  });
}
