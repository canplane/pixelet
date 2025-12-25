import { VOXEL_TYPES, type TileGrids } from '../baking/tile-baker';
import { VOXEL_COLORS } from '../../config/map-theme';

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

  // ✨ [New] 추가된 타입 매핑
  [VOXEL_TYPES.INSTITUTIONAL]: hexToRgb(VOXEL_COLORS.INSTITUTIONAL),
  [VOXEL_TYPES.PARKING]: hexToRgb(VOXEL_COLORS.PARKING),
  [VOXEL_TYPES.SPORTS]: hexToRgb(VOXEL_COLORS.SPORTS),
});

/**
 * [tile-debug]
 * 생성된 그리드 데이터를 바이너리 파일(.bin)로 저장합니다.
 */
export function downloadGridAsBinary(
  grid: Uint8Array,
  origin: { x: number; z: number },
  resolution: number,
  filename?: string,
) {
  const HEADER_SIZE = 12;
  const combinedBuffer = new Uint8Array(HEADER_SIZE + grid.length);
  const view = new DataView(combinedBuffer.buffer);

  view.setInt32(0, resolution, true);
  view.setInt32(4, Math.floor(origin.x), true);
  view.setInt32(8, Math.floor(origin.z), true);

  combinedBuffer.set(grid, HEADER_SIZE);

  const blob = new Blob([combinedBuffer], { type: 'application/octet-stream' });
  const finalName =
    filename || `tile_${Math.floor(origin.x)}_${Math.floor(origin.z)}.bin`;
  saveFile(blob, finalName);
}

/**
 * [tile-debug]
 * 단일 채널 그리드 데이터를 PNG 이미지로 변환하여 시각적으로 검증합니다.
 */
export function downloadGridAsImage(
  grid: Uint8Array,
  resolution: number,
  origin: { x: number; z: number },
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

  for (let i = 0; i < grid.length; i++) {
    const voxelValue = grid[i];
    // 매핑되지 않은 값은 기본 땅 색상으로 처리 (Safety fallback)
    const rgb = colorMap[voxelValue] || colorMap[VOXEL_TYPES.BASE_LAND];

    const idx = i * 4;
    imageData.data[idx] = rgb[0];
    imageData.data[idx + 1] = rgb[1];
    imageData.data[idx + 2] = rgb[2];
    imageData.data[idx + 3] = 255;
  }

  ctx.putImageData(imageData, 0, 0);

  const finalName =
    filename ||
    `debug_tile_${Math.floor(origin.x)}_${Math.floor(origin.z)}.png`;

  canvas.toBlob((blob) => {
    if (blob) saveFile(blob, finalName);
  });
}

/**
 * ✨ [New] [tile-debug]
 * 4개의 레이어(Base, Water, Transport, Structure)를 우선순위에 따라 합성하여
 * 최종 렌더링 결과와 동일한 하나의 PNG 이미지를 생성합니다.
 * * Priority: Structure > Transport > Water > Base
 */
export function downloadMergedGridAsImage(
  grids: TileGrids,
  resolution: number,
  origin: { x: number; z: number },
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

  for (let i = 0; i < length; i++) {
    // 1. 각 레이어의 값 추출
    const tBase = grids.base[i];
    const tWater = grids.water[i];
    const tTrans = grids.transport[i];
    const tStruct = grids.structure[i];

    // 2. 우선순위 합성 (Shader Logic과 동일)
    let finalType = tBase;

    if (tWater !== VOXEL_TYPES.EMPTY) finalType = tWater;
    if (tTrans !== VOXEL_TYPES.EMPTY) finalType = tTrans;
    if (tStruct !== VOXEL_TYPES.EMPTY) finalType = tStruct;

    // 3. 색상 매핑
    const rgb = colorMap[finalType] || colorMap[VOXEL_TYPES.BASE_LAND];
    const idx = i * 4;
    imageData.data[idx] = rgb[0];
    imageData.data[idx + 1] = rgb[1];
    imageData.data[idx + 2] = rgb[2];
    imageData.data[idx + 3] = 255;
  }

  ctx.putImageData(imageData, 0, 0);

  const finalName =
    filename ||
    `merged_tile_${Math.floor(origin.x)}_${Math.floor(origin.z)}.png`;

  canvas.toBlob((blob) => {
    if (blob) saveFile(blob, finalName);
  });
}

function saveFile(blob: Blob, filename: string) {
  const url = URL.createObjectURL(blob);
  const a = document.createElement('a');
  a.href = url;
  a.download = filename;
  a.click();
  URL.revokeObjectURL(url);
}
