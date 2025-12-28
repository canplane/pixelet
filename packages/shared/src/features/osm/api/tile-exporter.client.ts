/**
 * @file tile-debug.ts
 * 엔진 내부의 그리드 데이터(Uint8Array)를 바이너리 또는 이미지 파일로 추출하여
 * 데이터의 정합성을 시각적으로 검증하는 도구입니다.
 */

import {
  VOXEL_COLORS,
  VOXEL_TYPES,
} from '@pixelet/shared/entities/voxel/model/voxels';
import type { TileGrids } from '@pixelet/shared/entities/osm/model/tiles';

/**
 * [hex-to-rgb]
 * HEX 색상 코드를 Canvas ImageData에서 사용할 수 있는 RGB 배열로 변환합니다.
 */
const hexToRgb = (hex: string): [number, number, number] => {
  const r = parseInt(hex.slice(1, 3), 16);
  const g = parseInt(hex.slice(3, 5), 16);
  const b = parseInt(hex.slice(5, 7), 16);
  return [r, g, b];
};

/**
 * [Helper] 컬러 맵 생성
 * VOXEL_COLORS 상수를 RGB로 변환하여 매핑 테이블을 구성합니다.
 * 단일 채널 및 병합 이미지 생성 시 공통으로 사용됩니다.
 */
const getColorMap = (): Record<number, [number, number, number]> => ({
  [VOXEL_TYPES.EMPTY]: hexToRgb(VOXEL_COLORS.EMPTY),
  [VOXEL_TYPES.WATER]: hexToRgb(VOXEL_COLORS.WATER),
  [VOXEL_TYPES.ROAD]: hexToRgb(VOXEL_COLORS.ROAD),
  [VOXEL_TYPES.BUILDING]: hexToRgb(VOXEL_COLORS.BUILDING),
  [VOXEL_TYPES.GREEN_DEEP]: hexToRgb(VOXEL_COLORS.GREEN_DEEP),
  [VOXEL_TYPES.GREEN_LIGHT]: hexToRgb(VOXEL_COLORS.GREEN_LIGHT),
  [VOXEL_TYPES.BASE_LAND]: hexToRgb(VOXEL_COLORS.BASE_LAND),
  [VOXEL_TYPES.RESIDENTIAL]: hexToRgb(VOXEL_COLORS.RESIDENTIAL),
  [VOXEL_TYPES.COMMERCIAL]: hexToRgb(VOXEL_COLORS.COMMERCIAL),
  [VOXEL_TYPES.INDUSTRIAL]: hexToRgb(VOXEL_COLORS.INDUSTRIAL),
  [VOXEL_TYPES.SAND]: hexToRgb(VOXEL_COLORS.SAND),
  [VOXEL_TYPES.PEDESTRIAN]: hexToRgb(VOXEL_COLORS.PEDESTRIAN),
  [VOXEL_TYPES.RAIL]: hexToRgb(VOXEL_COLORS.RAIL),

  // ✨ [New] 행정 구역 및 시설 단지 타입 매핑
  [VOXEL_TYPES.INSTITUTIONAL]: hexToRgb(VOXEL_COLORS.INSTITUTIONAL),
  [VOXEL_TYPES.PARKING]: hexToRgb(VOXEL_COLORS.PARKING),
  [VOXEL_TYPES.SPORTS]: hexToRgb(VOXEL_COLORS.SPORTS),
});

/**
 * [tile-debug]
 * 생성된 그리드 데이터를 바이너리 파일(.bin)로 저장합니다.
 * 파일 헤더에는 resolution과 origin 정보가 포함되어 서버/클라이언트 간 데이터 검증에 사용됩니다.
 */
export function downloadGridAsBinary(
  grid: Uint8Array,
  origin: { x: number; y: number },
  resolution: number,
  filename?: string,
) {
  const HEADER_SIZE = 12; // resolution(4) + originX(4) + originY(4)
  const combinedBuffer = new Uint8Array(HEADER_SIZE + grid.length);
  const view = new DataView(combinedBuffer.buffer);

  view.setInt32(0, resolution, true);
  view.setInt32(4, Math.floor(origin.x), true);
  view.setInt32(8, Math.floor(origin.y), true);

  combinedBuffer.set(grid, HEADER_SIZE);

  const blob = new Blob([combinedBuffer], { type: 'application/octet-stream' });
  const finalName =
    filename || `tile_${Math.floor(origin.x)}_${Math.floor(origin.y)}.bin`;
  saveFile(blob, finalName);
}

/**
 * [tile-debug]
 * 단일 채널 그리드 데이터를 PNG 이미지로 변환하여 시각적으로 검증합니다.
 * 각 채널(Base, Water 등)의 데이터가 베이커에서 의도한 대로 구워졌는지 확인하는 용도입니다.
 */
export function downloadGridAsImage(
  grid: Uint8Array,
  resolution: number,
  origin: { x: number; y: number },
  filename?: string,
) {
  if (typeof document === 'undefined') return;

  const canvas = document.createElement('canvas');
  canvas.width = resolution;
  canvas.height = resolution;
  const ctx = canvas.getContext('2d');
  if (!ctx) return;

  const colorMap = getColorMap();
  const imageData = ctx.createImageData(resolution, resolution);

  for (let y = 0; y < resolution; y++) {
    for (let x = 0; x < resolution; x++) {
      // 💡 Canvas는 위에서 아래로(0 -> res), 데이터는 아래에서 위로 쌓여있음
      // 따라서 데이터를 읽을 때 Y축을 뒤집어서 읽어야 실제 지도 방향과 일치함
      const dataIdx = (resolution - 1 - y) * resolution + x;
      const voxelValue = grid[dataIdx];

      // 매핑되지 않은 값은 기본 땅 색상으로 처리 (Safety fallback)
      const rgb = colorMap[voxelValue] || colorMap[VOXEL_TYPES.BASE_LAND];
      const canvasIdx = (y * resolution + x) * 4;

      imageData.data[canvasIdx] = rgb[0];
      imageData.data[canvasIdx + 1] = rgb[1];
      imageData.data[canvasIdx + 2] = rgb[2];
      imageData.data[canvasIdx + 3] = 255;
    }
  }

  ctx.putImageData(imageData, 0, 0);

  const finalName =
    filename ||
    `debug_tile_${Math.floor(origin.x)}_${Math.floor(origin.y)}.png`;

  canvas.toBlob((blob) => {
    if (blob) saveFile(blob, finalName);
  });
}

/**
 * [tile-debug]
 * 4개의 레이어(Base, Water, Transport, Structure)를 우선순위에 따라 합성하여
 * 최종 렌더링 결과와 동일한 하나의 PNG 이미지를 생성합니다.
 * * [Composition Logic]
 * 쉐이더의 렌더링 파이프라인과 동일한 우선순위를 따릅니다.
 * Priority: Structure(Building) > Transport(Road/Parking) > Water > Base(Landuse)
 */
export function downloadMergedGridAsImage(
  grids: TileGrids,
  resolution: number,
  origin: { x: number; y: number },
  filename?: string,
) {
  if (typeof document === 'undefined') return;

  const canvas = document.createElement('canvas');
  canvas.width = resolution;
  canvas.height = resolution;
  const ctx = canvas.getContext('2d');
  if (!ctx) return;

  const colorMap = getColorMap();
  const imageData = ctx.createImageData(resolution, resolution);
  const length = resolution * resolution;

  for (let y = 0; y < resolution; y++) {
    for (let x = 0; x < resolution; x++) {
      // 합성 함수에서도 동일한 인덱스 반전 적용
      const i = (resolution - 1 - y) * resolution + x;

      // 1. 각 레이어의 개별 복셀 값 추출
      const tBase = grids.base[i];
      const tWater = grids.water[i];
      const tTrans = grids.transport[i];
      const tStruct = grids.structure[i];

      // 2. 우선순위 합성 (Shader Logic과 1:1 대응)
      // 하위 레이어에서 상위 레이어로 덮어쓰기 수행
      let finalType = tBase;

      if (tWater !== VOXEL_TYPES.EMPTY) finalType = tWater;
      if (tTrans !== VOXEL_TYPES.EMPTY) finalType = tTrans;
      if (tStruct !== VOXEL_TYPES.EMPTY) finalType = tStruct;

      // 3. 합성된 결과에 따른 색상 매핑
      const rgb = colorMap[finalType] || colorMap[VOXEL_TYPES.BASE_LAND];
      const canvasIdx = (y * resolution + x) * 4;

      imageData.data[canvasIdx] = rgb[0];
      imageData.data[canvasIdx + 1] = rgb[1];
      imageData.data[canvasIdx + 2] = rgb[2];
      imageData.data[canvasIdx + 3] = 255;
    }
  }

  ctx.putImageData(imageData, 0, 0);

  const finalName =
    filename ||
    `merged_tile_${Math.floor(origin.x)}_${Math.floor(origin.y)}.png`;

  canvas.toBlob((blob) => {
    if (blob) saveFile(blob, finalName);
  });
}

/**
 * [Internal Helper] 파일 브라우저 다운로드 트리거
 */
function saveFile(blob: Blob, filename: string) {
  const url = URL.createObjectURL(blob);
  const a = document.createElement('a');
  a.href = url;
  a.download = filename;
  a.click();
  URL.revokeObjectURL(url);
}
