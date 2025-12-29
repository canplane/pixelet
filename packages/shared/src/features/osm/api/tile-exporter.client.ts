/**
 * @file tile-debug.ts
 * 엔진 내부의 그리드 데이터(Uint8Array)를 시각적 이미지(PNG) 또는 바이너리(.bin)로 추출합니다.
 * 베이킹 엔진(Baker)에서 생성된 복셀 데이터가 실제 지형과 일치하는지 검증하는 용도입니다.
 * * [설계 변경: Standard Raster Order]
 * 데이터가 이제 표준 이미지 좌표계(Top-Down, 북->남)를 따르므로,
 * Canvas API와 방향이 일치합니다. 별도의 Y축 반전(Flip) 없이 순차적으로 그립니다.
 */

import {
  VOXEL_COLORS,
  VOXEL_TYPES,
} from '@pixelet/shared/entities/voxel/model/voxels';
import type { VoxelLayers } from '@pixelet/shared/entities/osm/model/tiles';

/**
 * [hex-to-rgb]
 * HEX(#RRGGBB 또는 RRGGBB)를 [r, g, b]로 변환
 */
const hexToRgb = (hex: string): [number, number, number] => {
  const cleanHex = hex.startsWith('#') ? hex.slice(1) : hex;
  const r = parseInt(cleanHex.slice(0, 2), 16);
  const g = parseInt(cleanHex.slice(2, 4), 16);
  const b = parseInt(cleanHex.slice(4, 6), 16);
  return [r, g, b];
};

/**
 * [getColorMap]
 * 복셀 타입별 정의된 색상을 매핑 테이블로 구축합니다.
 * 쉐이더의 팔레트 루크업(Palette Lookup)과 동일한 논리를 가집니다.
 */
const getColorMap = (): Record<number, [number, number, number]> => ({
  [VOXEL_TYPES.UNKNOWN]: hexToRgb(VOXEL_COLORS.UNKNOWN),
  [VOXEL_TYPES.BASE_LAND]: hexToRgb(VOXEL_COLORS.BASE_LAND),
  [VOXEL_TYPES.WATER]: hexToRgb(VOXEL_COLORS.WATER),
  [VOXEL_TYPES.GREEN_DEEP]: hexToRgb(VOXEL_COLORS.GREEN_DEEP),
  [VOXEL_TYPES.GREEN_LIGHT]: hexToRgb(VOXEL_COLORS.GREEN_LIGHT),
  [VOXEL_TYPES.SAND]: hexToRgb(VOXEL_COLORS.SAND),
  [VOXEL_TYPES.RESIDENTIAL]: hexToRgb(VOXEL_COLORS.RESIDENTIAL),
  [VOXEL_TYPES.COMMERCIAL]: hexToRgb(VOXEL_COLORS.COMMERCIAL),
  [VOXEL_TYPES.INDUSTRIAL]: hexToRgb(VOXEL_COLORS.INDUSTRIAL),
  [VOXEL_TYPES.INSTITUTIONAL]: hexToRgb(VOXEL_COLORS.INSTITUTIONAL),
  [VOXEL_TYPES.SPORTS]: hexToRgb(VOXEL_COLORS.SPORTS),
  [VOXEL_TYPES.ROAD]: hexToRgb(VOXEL_COLORS.ROAD),
  [VOXEL_TYPES.PEDESTRIAN]: hexToRgb(VOXEL_COLORS.PEDESTRIAN),
  [VOXEL_TYPES.RAIL]: hexToRgb(VOXEL_COLORS.RAIL),
  [VOXEL_TYPES.PARKING]: hexToRgb(VOXEL_COLORS.PARKING),
  [VOXEL_TYPES.BUILDING]: hexToRgb(VOXEL_COLORS.BUILDING),
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
       * 🎯 [Standard Order]
       * 데이터가 Top-Down(북->남)으로 정렬되어 있으므로, Canvas 좌표계와 일치합니다.
       * 별도의 Y-Flip 로직 없이 인덱스를 그대로 매핑합니다.
       */
      const dataIdx = y * resolution + x; // (resolution - 1 - y) 제거됨
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
 * 쉐이더(Fragment Shader)의 합성 로직을 그대로 모방하여 이미지를 생성합니다.
 * 1. Structure (최상단: 건물)
 * 2. Transport (도로/주차장)
 * 3. Water (수계)
 * 4. Base (최하단: 지형/구역)
 */
export function downloadMergedGridAsImage(
  layers: VoxelLayers,
  resolution: number,
  origin: { x: number; y: number },
  filename?: string,
) {
  if (typeof document === 'undefined' || !layers) return;

  const expectedLength = resolution * resolution;
  const channels: (keyof VoxelLayers)[] = [
    'base',
    'water',
    'transport',
    'structure',
  ];

  // 1. 사이즈 검증
  for (const channel of channels) {
    if (!layers[channel] || layers[channel].length !== expectedLength) {
      console.error(
        `[TileDebug] ${channel} mismatch! Actual: ${layers[channel]?.length}, Expected: ${expectedLength}`,
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

  // 🎯 쉐이더의 Fallback 색상 (보통 Base Land 색상)
  const FALLBACK_COLOR = colorMap[VOXEL_TYPES.BASE_LAND];

  for (let y = 0; y < resolution; y++) {
    for (let x = 0; x < resolution; x++) {
      /**
       * 🎯 [Standard Order]
       * 데이터와 캔버스 모두 Top-Down(북->남) 좌표계를 따르므로 Flip 없이 매핑합니다.
       */
      const dataIdx = y * resolution + x; // (resolution - 1 - y) 제거됨

      const tBase = layers.base[dataIdx] ?? 0;
      const tWater = layers.water[dataIdx] ?? 0;
      const tTrans = layers.transport[dataIdx] ?? 0;
      const tStruct = layers.structure[dataIdx] ?? 0;

      /**
       * 🎯 [Shader Logic Mirroring]
       * 쉐이더 코드:
       * vec3 finalColor = getTypeColor(tBase);
       * if (tWater > TYPE_UNKNOWN) finalColor = getTypeColor(tWater);
       * ...
       * getTypeColor 내부: 매칭 안되면 return uColorBaseLand;
       */

      // 1. Base Layer 처리 (0이면 BaseLand로 Fallback)
      let finalType =
        tBase === VOXEL_TYPES.UNKNOWN ? VOXEL_TYPES.BASE_LAND : tBase;

      // 2. Layer Overwrite (0보다 클 때만 덮어씀)
      if (tWater > VOXEL_TYPES.UNKNOWN) finalType = tWater;
      if (tTrans > VOXEL_TYPES.UNKNOWN) finalType = tTrans;
      if (tStruct > VOXEL_TYPES.UNKNOWN) finalType = tStruct;

      // 3. 색상 매핑
      const rgb = colorMap[finalType] || FALLBACK_COLOR;

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
