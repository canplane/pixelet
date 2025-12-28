/**
 * @file tile-debug.ts
 * 엔진 내부의 그리드 데이터(Uint8Array)를 시각적 이미지(PNG) 또는 바이너리(.bin)로 추출합니다.
 * 베이킹 엔진(Baker)에서 생성된 복셀 데이터가 실제 지형과 일치하는지 검증하는 용도입니다.
 * * [핵심 설계: Y축 반전(Flip)]
 * Canvas API는 좌측 상단(0,0)에서 시작하지만, 엔진의 데이터는 남쪽에서 북쪽으로 쌓입니다.
 * 따라서 추출 시 Y축을 반전시켜야 실제 지도와 동일한 방향의 이미지를 얻을 수 있습니다.
 */

import {
  VOXEL_COLORS,
  VOXEL_TYPES,
} from '@pixelet/shared/entities/voxel/model/voxels';
import type { TileGrids } from '@pixelet/shared/entities/osm/model/tiles';

/**
 * [hex-to-rgb]
 * HEX 색상을 ImageData(RGBA)에서 소비 가능한 RGB 배열로 변환합니다.
 */
const hexToRgb = (hex: string): [number, number, number] => {
  const r = parseInt(hex.slice(1, 3), 16);
  const g = parseInt(hex.slice(3, 5), 16);
  const b = parseInt(hex.slice(5, 7), 16);
  return [r, g, b];
};

/**
 * [getColorMap]
 * 복셀 타입별 정의된 색상을 매핑 테이블로 구축합니다.
 * 쉐이더의 팔레트 루크업(Palette Lookup)과 동일한 논리를 가집니다.
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
  [VOXEL_TYPES.INSTITUTIONAL]: hexToRgb(VOXEL_COLORS.INSTITUTIONAL),
  [VOXEL_TYPES.PARKING]: hexToRgb(VOXEL_COLORS.PARKING),
  [VOXEL_TYPES.SPORTS]: hexToRgb(VOXEL_COLORS.SPORTS),
});

/**
 * [downloadGridAsBinary]
 * 그리드 원본 데이터를 바이너리 파일로 저장합니다.
 * 헤더(12 bytes): Resolution(4) + OriginX(4) + OriginY(4) 정보가 포함되어 타일 재현이 가능합니다.
 */
export function downloadGridAsBinary(
  grid: Uint8Array,
  origin: { x: number; y: number },
  resolution: number,
  filename?: string,
) {
  const HEADER_SIZE = 12;
  const combinedBuffer = new Uint8Array(HEADER_SIZE + grid.length);
  const view = new DataView(combinedBuffer.buffer);

  // 🎯 Little Endian 방식으로 헤더 데이터 주입
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
 * [downloadGridAsImage]
 * 단일 채널(예: Water 전용) 그리드를 시각화합니다.
 * 특정 레이어의 베이킹 무결성을 점검할 때 사용합니다.
 */
export function downloadGridAsImage(
  grid: Uint8Array,
  resolution: number,
  origin: { x: number; y: number },
  filename?: string,
) {
  if (typeof document === 'undefined' || !grid) return;

  // 🎯 [Safety Guard] 배열 크기 불일치 시 런타임 에러 방지 및 로깅
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

  for (let y = 0; y < resolution; y++) {
    for (let x = 0; x < resolution; x++) {
      /**
       * 🎯 [Y-axis Flip Logic]
       * 데이터 인덱스: 아래 -> 위 (남 -> 북)
       * Canvas 좌표: 위 -> 아래 (북 -> 남)
       * 상하 반전 처리를 통해 실제 지도 뷰와 일치시킵니다.
       */
      const dataIdx = (resolution - 1 - y) * resolution + x;
      const voxelValue = grid[dataIdx];

      const rgb = colorMap[voxelValue] || colorMap[VOXEL_TYPES.BASE_LAND];
      const canvasIdx = (y * resolution + x) * 4;

      imageData.data[canvasIdx] = rgb[0];
      imageData.data[canvasIdx + 1] = rgb[1];
      imageData.data[canvasIdx + 2] = rgb[2];
      imageData.data[canvasIdx + 3] = 255; // Alpha
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
 * [downloadMergedGridAsImage]
 * 4개의 레이어(Base, Water, Transport, Structure)를 합성하여 최종 타일 이미지를 생성합니다.
 * * [Layer Composition Priority]
 * 쉐이더의 연산 우선순위와 동일하게 동작합니다:
 * 1. Structure (최상단: 건물)
 * 2. Transport (도로/주차장)
 * 3. Water (수계)
 * 4. Base (최하단: 지형/구역)
 */
export function downloadMergedGridAsImage(
  grids: TileGrids,
  resolution: number,
  origin: { x: number; y: number },
  filename?: string,
) {
  if (typeof document === 'undefined' || !grids) return;

  // 🎯 [무결성 체크] 모든 그리드 채널의 크기가 해상도와 일치하는지 전수 검사
  const expectedLength = resolution * resolution;
  const channels: (keyof TileGrids)[] = [
    'base',
    'water',
    'transport',
    'structure',
  ];

  for (const channel of channels) {
    if (!grids[channel] || grids[channel].length !== expectedLength) {
      console.error(
        `[TileDebug] ${channel} channel size mismatch! 
         Actual: ${grids[channel]?.length ?? 0}, Expected: ${expectedLength} (res: ${resolution})`,
      );
      // 🚨 여기서 return 하거나 해상도를 실제 데이터에 맞게 강제 조정해야 함
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

  for (let y = 0; y < resolution; y++) {
    for (let x = 0; x < resolution; x++) {
      const i = (resolution - 1 - y) * resolution + x;

      // 🎯 [Safety Fallback] 만에 하나 undefined일 경우 EMPTY(0)로 처리
      const tBase = grids.base[i] ?? 0;
      const tWater = grids.water[i] ?? 0;
      const tTrans = grids.transport[i] ?? 0;
      const tStruct = grids.structure[i] ?? 0;

      let finalType = tBase;
      if (tWater !== VOXEL_TYPES.EMPTY) finalType = tWater;
      if (tTrans !== VOXEL_TYPES.EMPTY) finalType = tTrans;
      if (tStruct !== VOXEL_TYPES.EMPTY) finalType = tStruct;

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
