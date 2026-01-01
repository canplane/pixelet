/**
 * @file tile-exporter.ts
 * 엔진 내부의 그리드 데이터(Uint8Array)를 시각적 이미지(PNG) 또는 바이너리(.bin)로 추출합니다.
 * 베이킹 엔진(Baker)에서 생성된 복셀 데이터가 실제 지형과 일치하는지 검증하는 용도입니다.
 *
 * [Refactor Note]
 * 1. VOXEL_TYPES/COLORS를 순회하여 색상 맵을 자동 생성합니다.
 * 2. GRID_LAYER_KEYS를 순회하여 Sea 레이어를 포함한 모든 레이어를 동적으로 합성합니다.
 */

import {
  VOXEL_COLORS,
  VOXEL_TYPES,
} from '@pixelet/shared/entities/voxel/model/voxels';
import {
  GRID_LAYER_KEYS,
  type GridLayers,
} from '@pixelet/shared/entities/osm/model/tiles';
import {
  getTileBbox,
  type TileCoord,
} from '@pixelet/shared/entities/osm/lib/utils/tile-utils';
import { OSM_CONSTANTS } from '@pixelet/shared/entities/osm/model/osm-constants';

const { TILE_RESOLUTION: resolution } = OSM_CONSTANTS;

/**
 * [hex-to-rgb]
 * HEX(#RRGGBB 또는 RRGGBB)를 [r, g, b]로 변환
 */
const hexToRgb = (hex: string): [number, number, number] => {
  // 'transparent' 등의 예외 처리 (검은색 반환)
  if (!hex || !hex.startsWith('#')) return [0, 0, 0];

  const cleanHex = hex.slice(1);
  const r = parseInt(cleanHex.slice(0, 2), 16);
  const g = parseInt(cleanHex.slice(2, 4), 16);
  const b = parseInt(cleanHex.slice(4, 6), 16);
  return [r, g, b];
};

/**
 * [getColorMap]
 * 🚀 [Automation] 복셀 타입별 정의된 색상을 매핑 테이블로 구축합니다.
 * voxels.ts에 정의된 모든 타입을 순회하므로 유지보수가 필요 없습니다.
 */
const getColorMap = (): Record<number, [number, number, number]> => {
  const map: Record<number, [number, number, number]> = {};

  (Object.keys(VOXEL_TYPES) as (keyof typeof VOXEL_TYPES)[]).forEach((key) => {
    const typeId = VOXEL_TYPES[key];
    // Partial 방어를 위해 key casting
    const hexColor = VOXEL_COLORS[key as keyof typeof VOXEL_COLORS];

    // AIR(0)나 색상이 없는 경우 스킵
    if (hexColor) {
      map[typeId] = hexToRgb(hexColor);
    }
  });

  return map;
};

/**
 * [downloadGridAsBinary]
 * 그리드 원본 데이터를 바이너리 파일로 저장합니다.
 *
 * * [Header Specification] (Total 12 Bytes, Little Endian)
 * - 0~3 byte: Resolution (int32)
 * - 4~7 byte: Origin X (int32, World Meter)
 * - 8~11 byte: Origin Y (int32, World Meter)
 */
export function downloadGridAsBinary(
  coord: TileCoord,
  grid: Uint8Array,
  filename: string,
) {
  const HEADER_SIZE = 12;
  const combinedBuffer = new Uint8Array(HEADER_SIZE + grid.length);
  const view = new DataView(combinedBuffer.buffer);

  // [Coordinate Calculation]
  const bbox = getTileBbox(coord);

  // 🎯 Little Endian 방식으로 헤더 데이터 주입
  view.setInt32(0, resolution, true);
  view.setInt32(4, Math.floor(bbox.minX), true); // Origin X
  view.setInt32(8, Math.floor(bbox.minY), true); // Origin Y

  combinedBuffer.set(grid, HEADER_SIZE);

  const blob = new Blob([combinedBuffer], { type: 'application/octet-stream' });
  saveFile(blob, filename);
}

/**
 * [downloadGridAsImage]
 * 단일 채널(예: Sea, Water) 그리드를 시각화합니다.
 */
export function downloadGridAsImage(
  coord: TileCoord,
  grid: Uint8Array,
  filename: string,
) {
  if (typeof document === 'undefined' || !grid) return;

  // [Safety Guard]
  if (grid.length !== resolution * resolution) {
    console.error(
      `[TileDebug] Data size mismatch! Array: ${grid.length}, Expected: ${resolution * resolution}`,
    );
    return;
  }

  const canvas = document.createElement('canvas');
  canvas.width = resolution;
  canvas.height = resolution;
  const ctx = canvas.getContext('2d');
  if (!ctx) return;

  const colorMap = getColorMap();
  const imageData = ctx.createImageData(resolution, resolution);

  for (let i = 0; i < grid.length; i++) {
    const voxelValue = grid[i];
    // 값이 없으면 검은색, 있으면 매핑된 색상, 매핑 없으면 기본 땅 색상 fallback
    const rgb = colorMap[voxelValue] ||
      colorMap[VOXEL_TYPES.BASE_LAND] || [0, 0, 0];
    const canvasIdx = i * 4;

    imageData.data[canvasIdx] = rgb[0];
    imageData.data[canvasIdx + 1] = rgb[1];
    imageData.data[canvasIdx + 2] = rgb[2];
    imageData.data[canvasIdx + 3] = 255; // Alpha
  }

  ctx.putImageData(imageData, 0, 0);

  canvas.toBlob((blob) => {
    if (blob) saveFile(blob, filename);
  });
}

/**
 * [downloadMergedGridAsImage]
 * 🚀 [Layer Automation] GRID_LAYER_KEYS에 정의된 모든 레이어를 합성합니다.
 * 쉐이더의 Painter's Algorithm을 그대로 JS로 구현하여 최종 결과물을 시뮬레이션합니다.
 */
export function downloadMergedGridAsImage(
  coord: TileCoord,
  layers: GridLayers,
  filename: string,
) {
  if (typeof document === 'undefined' || !layers) return;

  // 1. 데이터 무결성 검증 (Loop)
  const expectedLength = resolution * resolution;
  for (const key of GRID_LAYER_KEYS) {
    if (!layers[key] || layers[key].length !== expectedLength) {
      console.error(
        `[TileDebug] ${key} mismatch! Actual: ${layers[key]?.length}, Expected: ${expectedLength}`,
      );
      return;
    }
  }

  const canvas = document.createElement('canvas');
  canvas.width = resolution;
  canvas.height = resolution;
  const ctx = canvas.getContext('2d');
  if (!ctx) return;

  const colorMap = getColorMap();
  const imageData = ctx.createImageData(resolution, resolution);

  // 기본값: BaseLand 색상
  const FALLBACK_RGB = colorMap[VOXEL_TYPES.BASE_LAND];

  // 2. 픽셀 단위 합성
  for (let i = 0; i < expectedLength; i++) {
    /**
     * [Painter's Algorithm 정교화]
     * 0(AIR)으로 시작하여 GRID_LAYER_KEYS 순서대로 유효한 값을 덮어씌웁니다.
     */
    let finalType: number = VOXEL_TYPES.AIR;

    for (const key of GRID_LAYER_KEYS) {
      const val = layers[key][i];
      // UNKNOWN(1)보다 큰 유효한 데이터가 있을 때만 해당 레이어 값으로 갱신
      if (val > VOXEL_TYPES.UNKNOWN) {
        finalType = val;
      }
    }

    /**
     * 3. 최종 색상 결정
     * 모든 레이어를 검사했는데도 데이터가 없다면(AIR/UNKNOWN),
     * 시각적 결손 방지를 위해 BASE_LAND 색상을 적용합니다.
     */
    const rgb =
      finalType > VOXEL_TYPES.UNKNOWN
        ? colorMap[finalType] || FALLBACK_RGB
        : FALLBACK_RGB;

    const canvasIdx = i * 4;
    imageData.data[canvasIdx] = rgb[0];
    imageData.data[canvasIdx + 1] = rgb[1];
    imageData.data[canvasIdx + 2] = rgb[2];
    imageData.data[canvasIdx + 3] = 255;
  }

  ctx.putImageData(imageData, 0, 0);
  canvas.toBlob((blob) => {
    if (blob) saveFile(blob, filename);
  });
}

/**
 * [saveFile]
 * 생성된 Blob 데이터를 브라우저의 다운로드 트리거를 통해 로컬 파일로 저장합니다.
 */
function saveFile(blob: Blob, filename: string) {
  const url = URL.createObjectURL(blob);
  const a = document.createElement('a');
  a.href = url;
  a.download = filename;
  a.click();
  URL.revokeObjectURL(url);
}
