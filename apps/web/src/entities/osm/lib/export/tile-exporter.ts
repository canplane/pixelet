/**
 * @file tile-exporter.ts
 * 엔진 내부의 그리드 데이터(Uint8Array)를 시각적 이미지(PNG) 또는 바이너리(.bin)로 추출합니다.
 * 베이킹 엔진(Baker)에서 생성된 복셀 데이터가 실제 지형과 일치하는지 검증하는 용도입니다.
 *
 * [Refactor Note]
 * 외부에서 절대 좌표(Origin)를 주입받던 방식에서, 타일 인덱스(Index)만 받으면
 * 내부에서 필요한 좌표를 역산하는 방식으로 변경하여 모듈 독립성을 확보했습니다.
 */

import {
  VOXEL_COLORS,
  VOXEL_TYPES,
} from '@pixelet/shared/entities/voxel/model/voxels';
import type { GridLayers } from '@pixelet/shared/entities/osm/model/tiles';
// 🚀 [Refactor] 바이너리 헤더 작성을 위한 월드 좌표 계산 유틸리티
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
  const cleanHex = hex.startsWith('#') ? hex.slice(1) : hex;
  const r = parseInt(cleanHex.slice(0, 2), 16);
  const g = parseInt(cleanHex.slice(2, 4), 16);
  const b = parseInt(cleanHex.slice(4, 6), 16);
  return [r, g, b];
};

/**
 * [getColorMap]
 * 복셀 타입별 정의된 색상을 매핑 테이블로 구축합니다.
 * 쉐이더의 팔레트 루크업(Palette Lookup)과 동일한 색상을 사용하여 시각적 정합성을 맞춥니다.
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
 *
 * * [Header Specification] (Total 12 Bytes, Little Endian)
 * - 0~3 byte: Resolution (int32)
 * - 4~7 byte: Origin X (int32, World Meter)
 * - 8~11 byte: Origin Y (int32, World Meter)
 *
 * @param coord - 타일 인덱스 { tx, ty } (Origin 계산용)
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
  // 바이너리 파일 헤더에 '이 타일이 세계 어디에 위치하는지' 기록하기 위해 절대 좌표를 계산합니다.
  const bbox = getTileBbox(coord);

  // 🎯 Little Endian 방식으로 헤더 데이터 주입 (표준)
  view.setInt32(0, resolution, true);
  view.setInt32(4, Math.floor(bbox.minX), true); // Calculated Origin X
  view.setInt32(8, Math.floor(bbox.minY), true); // Calculated Origin Y

  combinedBuffer.set(grid, HEADER_SIZE);

  const blob = new Blob([combinedBuffer], { type: 'application/octet-stream' });
  saveFile(blob, filename);
}

/**
 * [downloadGridAsImage]
 * 단일 채널(예: Water 전용) 그리드를 시각화합니다.
 * 특정 레이어의 베이킹 무결성을 디버깅할 때 유용합니다.
 */
export function downloadGridAsImage(
  coord: TileCoord,
  grid: Uint8Array,
  filename: string,
) {
  if (typeof document === 'undefined' || !grid) return;

  // [Safety Guard] 데이터 크기 검증
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
      // 🎯 [Standard Order] Top-Down
      // 데이터가 표준 이미지 좌표계(북->남)로 저장되어 있으므로 Y-Flip 없이 순차 매핑합니다.
      const dataIdx = y * resolution + x;
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

  canvas.toBlob((blob) => {
    if (blob) saveFile(blob, filename);
  });
}

/**
 * [downloadMergedGridAsImage]
 * 4개의 레이어(Base, Water, Transport, Structure)를 합성하여 최종 타일 이미지를 생성합니다.
 * 실제 렌더링 파이프라인(Fragment Shader)과 동일한 우선순위 로직을 수행합니다.
 */
export function downloadMergedGridAsImage(
  coord: TileCoord,
  layers: GridLayers,
  filename: string,
) {
  if (typeof document === 'undefined' || !layers) return;

  const expectedLength = resolution * resolution;
  const channels: (keyof GridLayers)[] = [
    'base',
    'water',
    'transport',
    'structure',
  ];

  // 모든 채널의 데이터 무결성 검증
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
  const FALLBACK_COLOR = colorMap[VOXEL_TYPES.BASE_LAND];

  for (let y = 0; y < resolution; y++) {
    for (let x = 0; x < resolution; x++) {
      const dataIdx = y * resolution + x;

      const tBase = layers.base[dataIdx] ?? 0;
      const tWater = layers.water[dataIdx] ?? 0;
      const tTrans = layers.transport[dataIdx] ?? 0;
      const tStruct = layers.structure[dataIdx] ?? 0;

      /**
       * 🎯 [Shader Logic Simulation]
       * Fragment Shader의 레이어 합성 로직을 그대로 모방합니다.
       * 1. Base Layer를 기본값으로 설정
       * 2. Water, Transport, Structure 순으로 값이 존재하면(> UNKNOWN) 덮어씌움
       */
      let finalType =
        tBase === VOXEL_TYPES.UNKNOWN ? VOXEL_TYPES.BASE_LAND : tBase;

      if (tWater > VOXEL_TYPES.UNKNOWN) finalType = tWater;
      if (tTrans > VOXEL_TYPES.UNKNOWN) finalType = tTrans;
      if (tStruct > VOXEL_TYPES.UNKNOWN) finalType = tStruct;

      const rgb = colorMap[finalType] || FALLBACK_COLOR;

      const canvasIdx = (y * resolution + x) * 4;
      imageData.data[canvasIdx] = rgb[0];
      imageData.data[canvasIdx + 1] = rgb[1];
      imageData.data[canvasIdx + 2] = rgb[2];
      imageData.data[canvasIdx + 3] = 255;
    }
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
