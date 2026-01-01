/**
 * @file grid-packer.ts
 * [General Purpose Grid Packer]
 * 스키마 정의(tiles.ts)에 따라 동적으로 격자 데이터를 직렬화합니다.
 */

import { OSM_CONSTANTS } from '@pixelet/shared/entities/osm/model/osm-constants';
// 🎯 [Refactor] tiles.ts에서 SSOT(TILE_CHANNEL_KEYS, TILE_TERRAIN_KEYS)를 가져옵니다.
import {
  TILE_CHANNEL_KEYS,
  TILE_TERRAIN_KEYS,
  type TileGrids,
  type TileChannels,
  type TileTerrain,
} from '../model/tiles';
import { decompressBase64 } from './compression-utils';

const { TILE_RESOLUTION } = OSM_CONSTANTS;

// ----------------------------------------------------------------------
// 1. Packing Logic
// ----------------------------------------------------------------------

export const packTileGrids = (
  grids: TileGrids,
  gzipSync: (buf: Uint8Array, opt?: any) => any,
): string => {
  const { channels, terrain } = grids;

  // [Safety] water 채널 등이 누락되었을 경우를 대비해 channels.base 기준으로 해상도 산출
  const resolutionSq = channels.base.length;

  // 1. Calculate Total Size (Dynamic based on SSOT)
  // 8-bit Channels Total Size
  const channelsByteSize = resolutionSq * TILE_CHANNEL_KEYS.length;
  // 16-bit Terrain Total Size (2 bytes per cell)
  const terrainByteSize = resolutionSq * 2 * TILE_TERRAIN_KEYS.length;

  const totalSize = channelsByteSize + terrainByteSize;
  const combined = new Uint8Array(totalSize);

  let offset = 0;

  // 2. Pack 8-bit Channels (Base -> Water -> Transport -> Structure)
  for (const key of TILE_CHANNEL_KEYS) {
    const data = channels[key];
    combined.set(data, offset);
    offset += resolutionSq;
  }

  // 3. Pack 16-bit Terrain (Height)
  for (const key of TILE_TERRAIN_KEYS) {
    const data = terrain[key]; // Uint16Array
    // Uint16Array -> Uint8Array View 변환 후 복사 (Endianness: System default, usually LE)
    combined.set(new Uint8Array(data.buffer), offset);
    offset += resolutionSq * 2;
  }

  // 4. Compress (Level 9: CPU를 써서 트래픽 최소화)
  const compressed = gzipSync(combined, { level: 9 });
  return (compressed as any).toString('base64');
};

// ----------------------------------------------------------------------
// 2. Unpacking Logic (Client/Internal 사용)
// ----------------------------------------------------------------------

export const unpackTileGrids = async (
  b64: string,
  resolution: number = TILE_RESOLUTION,
): Promise<TileGrids> => {
  const decompressed = await decompressBase64(b64);
  const resolutionSq = resolution * resolution;
  let offset = 0;

  // 1. Unpack Channels
  const channels = {} as TileChannels;
  for (const key of TILE_CHANNEL_KEYS) {
    // Zero-copy slicing
    channels[key] = decompressed.subarray(offset, offset + resolutionSq);
    offset += resolutionSq;
  }

  // 2. Unpack Terrain
  const terrain = {} as TileTerrain;
  for (const key of TILE_TERRAIN_KEYS) {
    // ⚠️ [Alignment Safety] 16-bit 배열 생성 시 버퍼 오프셋 정렬 확인
    const subBuffer = decompressed.slice(
      offset,
      offset + resolutionSq * 2,
    ).buffer;
    terrain[key] = new Uint16Array(subBuffer);
    offset += resolutionSq * 2;
  }

  return {
    channels,
    terrain,
  };
};
