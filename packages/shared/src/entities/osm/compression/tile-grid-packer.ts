/**
 * @file grid-packer.ts
 * [General Purpose Grid Packer]
 * 하드코딩된 레이어 할당 로직을 제거하고, 스키마 정의에 따라 동적으로 격자 데이터를 처리합니다.
 */

import { OSM_CONSTANTS } from '@pixelet/shared/entities/osm/model/osm-constants';
import type { TileGrids, GridLayers, GridTerrain } from '../model/tiles';
import { decompressBase64 } from './compression-utils';

const { TILE_RESOLUTION } = OSM_CONSTANTS;

// ----------------------------------------------------------------------
// 1. Schema Definition (Configuration)
// ----------------------------------------------------------------------
/**
 * 🎯 [Maintenance Point]
 * 레이어가 추가되거나 변경되면 이 배열만 수정하면 됩니다.
 * 순서(Order)가 바이너리 오프셋을 결정하므로, 서버/클라이언트가 반드시 동일해야 합니다.
 */
const LAYER_KEYS: (keyof GridLayers)[] = [
  'base',
  'water',
  'transport',
  'structure',
];
const TERRAIN_KEYS: (keyof GridTerrain)[] = ['height'];

// ----------------------------------------------------------------------
// 2. Packing Logic
// ----------------------------------------------------------------------

export const packTileGrids = (
  grids: TileGrids,
  gzipSync: (buf: Uint8Array, opt?: any) => any,
): string => {
  const { layers, terrain } = grids;

  // 1. Calculate Total Size
  // (모든 그리드의 해상도가 동일하다고 가정: layers.base 기준)
  const resolutionSq = layers.base.length;

  // 8-bit Layers Total Size
  const layersByteSize = resolutionSq * LAYER_KEYS.length;
  // 16-bit Terrain Total Size (2 bytes per cell)
  const terrainByteSize = resolutionSq * 2 * TERRAIN_KEYS.length;

  const totalSize = layersByteSize + terrainByteSize;
  const combined = new Uint8Array(totalSize);

  let offset = 0;

  // 2. Pack 8-bit Layers (Loop)
  for (const key of LAYER_KEYS) {
    const data = layers[key];
    combined.set(data, offset);
    offset += resolutionSq;
  }

  // 3. Pack 16-bit Terrain (Loop)
  for (const key of TERRAIN_KEYS) {
    const data = terrain[key]; // Uint16Array
    // Uint16Array -> Uint8Array View 변환 후 복사 (Endianness: System default, usually LE)
    combined.set(new Uint8Array(data.buffer), offset);
    offset += resolutionSq * 2;
  }

  // 4. Compress
  const compressed = gzipSync(combined, { level: 9 });
  return (compressed as any).toString('base64');
};

// ----------------------------------------------------------------------
// 3. Unpacking Logic
// ----------------------------------------------------------------------

export const unpackTileGrids = async (
  b64: string,
  resolution: number = TILE_RESOLUTION,
): Promise<TileGrids> => {
  const decompressed = await decompressBase64(b64);
  const resolutionSq = resolution * resolution;
  let offset = 0;

  // 1. Unpack Layers
  const layers: Partial<GridLayers> = {};
  for (const key of LAYER_KEYS) {
    // Zero-copy slicing
    layers[key] = decompressed.subarray(offset, offset + resolutionSq);
    offset += resolutionSq;
  }

  // 2. Unpack Terrain
  const terrain: Partial<GridTerrain> = {};
  for (const key of TERRAIN_KEYS) {
    // Create new buffer slice for alignment safety
    const buffer = decompressed.slice(offset, offset + resolutionSq * 2).buffer;
    terrain[key] = new Uint16Array(buffer);
    offset += resolutionSq * 2;
  }

  return {
    layers: layers as GridLayers,
    terrain: terrain as GridTerrain,
  };
};
