/**
 * @file tile-exporter.ts
 * 엔진 내부의 그리드 데이터(Uint8Array)를 시각적 이미지(PNG) 또는 바이너리(.bin)로 추출합니다.
 * 베이킹 엔진(Baker)에서 생성된 복셀 데이터가 실제 지형과 일치하는지 검증하는 용도입니다.
 *
 * [Refactor Note]
 * 1. VOXEL_TYPES/COLORS를 순회하여 색상 맵을 자동 생성합니다.
 * 2. GRID_LAYER_KEYS를 순회하여 Sea 레이어를 포함한 모든 레이어를 동적으로 합성합니다.
 * 3. [Update] Major(Layer)/Minor(Type) 위계 구조에 맞춰 레이어별 독립 색상 매핑을 지원합니다.
 * 4. [Separation] 데이터 생성과 파일 저장(IO) 로직을 분리하여 범용성을 높였습니다.
 */

import {
  VOXEL_COLORS,
  VOXEL_TYPES,
} from '@pixelet/shared/entities/voxel/model/voxels';
import {
  GRID_LAYER_KEYS,
  type GridLayers,
} from '@pixelet/shared/entities/osm/model/tiles';
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
 * 🎯 [Update] 레이어별 독립된 ID 네임스페이스를 지원하기 위해 2차원 맵을 반환합니다.
 * 결과 구조: { sea: { 2: [r,g,b] }, base: { 2: [r,g,b] }, ... }
 */
const getColorMap = (): Record<
  string,
  Record<number, [number, number, number]>
> => {
  const map: Record<string, Record<number, [number, number, number]>> = {};

  // Major 카테고리(SEA, BASE, TRANSPORT 등)를 순회합니다.
  (Object.keys(VOXEL_TYPES) as (keyof typeof VOXEL_TYPES)[]).forEach(
    (majorKey) => {
      // COMMON은 스킵하고, 실제 레이어 키(sea, base 등)로 변환
      if (majorKey === 'COMMON') return;

      const layerKey = majorKey.toLowerCase();
      map[layerKey] = {};

      const types = VOXEL_TYPES[majorKey] as Record<string, number>;
      const colors = VOXEL_COLORS[
        majorKey as keyof typeof VOXEL_COLORS
      ] as Record<number, string>;

      (Object.keys(types) as (keyof typeof types)[]).forEach((minorKey) => {
        const typeId = types[minorKey];
        const hexColor = colors[typeId as any];

        // AIR(0)나 색상이 없는 경우 스킵
        if (hexColor && hexColor !== 'transparent') {
          map[layerKey][typeId] = hexToRgb(hexColor);
        }
      });
    },
  );

  return map;
};

/**
 * [Internal Helper] 캔버스 생성 및 픽셀 주입 공통 로직
 */
async function drawToBlob(
  pixelSource: (i: number, colorMap: any) => [number, number, number],
): Promise<Blob | null> {
  if (typeof document === 'undefined') return null;

  const canvas = document.createElement('canvas');
  canvas.width = resolution;
  canvas.height = resolution;
  const ctx = canvas.getContext('2d');
  if (!ctx) return null;

  const colorMap = getColorMap();
  const imageData = ctx.createImageData(resolution, resolution);

  for (let i = 0; i < resolution * resolution; i++) {
    const rgb = pixelSource(i, colorMap);
    const canvasIdx = i * 4;
    imageData.data[canvasIdx] = rgb[0];
    imageData.data[canvasIdx + 1] = rgb[1];
    imageData.data[canvasIdx + 2] = rgb[2];
    imageData.data[canvasIdx + 3] = 255;
  }

  ctx.putImageData(imageData, 0, 0);

  return new Promise((resolve) => {
    canvas.toBlob((blob) => resolve(blob), 'image/png');
  });
}

/**
 * [exportGridImage]
 * 단일 채널(예: Sea, Water) 그리드를 시각화하여 Blob으로 반환합니다.
 * 🎯 [Update] 레이어별 ID 스코핑을 위해 layerKey를 참조하도록 수정되었습니다.
 */
export async function exportGridImage(
  grid: Uint8Array,
  layerKey: keyof GridLayers,
): Promise<Blob | null> {
  return drawToBlob((i, colorMap) => {
    const voxelValue = grid[i];
    const layerColors = colorMap[layerKey] || {};
    const FALLBACK_RGB = colorMap.base[VOXEL_TYPES.BASE.BASE_LAND] || [
      242, 239, 233,
    ];

    // 값이 없으면 검은색, 있으면 매핑된 색상, 매핑 없으면 기본 땅 색상 fallback
    // COMMON.UNKNOWN(1) 보다 커야 유효한 값으로 간주
    return voxelValue > VOXEL_TYPES.COMMON.UNKNOWN
      ? layerColors[voxelValue] || FALLBACK_RGB
      : [0, 0, 0];
  });
}

/**
 * [exportMergedGridImage]
 * 🚀 [Layer Automation] GRID_LAYER_KEYS에 정의된 모든 레이어를 합성하여 Blob으로 반환합니다.
 * 쉐이더의 Painter's Algorithm을 그대로 JS로 구현하여 최종 결과물을 시뮬레이션합니다.
 */
export async function exportMergedGridImage(
  layers: GridLayers,
): Promise<Blob | null> {
  return drawToBlob((i, colorMap) => {
    /**
     * [Painter's Algorithm 정교화]
     * 0(AIR)으로 시작하여 GRID_LAYER_KEYS 순서대로 유효한 값을 덮어씌웁니다.
     */
    let finalRgb: [number, number, number] | null = null;
    const FALLBACK_RGB = colorMap.base[VOXEL_TYPES.BASE.BASE_LAND] || [
      242, 239, 233,
    ];

    for (const key of GRID_LAYER_KEYS) {
      const val = layers[key][i];
      // UNKNOWN(1)보다 큰 유효한 데이터가 있을 때만 해당 레이어 값으로 갱신
      if (val > VOXEL_TYPES.COMMON.UNKNOWN) {
        // 🎯 핵심: 값(val)뿐만 아니라 레이어(key) 정보도 함께 사용하여 2차원 맵에서 색상을 찾습니다.
        const layerColor = colorMap[key]?.[val];
        if (layerColor) finalRgb = layerColor;
      }
    }

    /**
     * 최종 색상 결정
     * 모든 레이어를 검사했는데도 데이터가 없다면(AIR/UNKNOWN),
     * 시각적 결손 방지를 위해 BASE_LAND 색상을 적용합니다.
     */
    return finalRgb || FALLBACK_RGB;
  });
}
