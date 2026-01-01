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
  const resSq = channels.base.length;

  // 1. Calculate Total Size (Dynamic based on SSOT)
  // 8-bit Channels Total Size
  const channelsByteSize = resSq * TILE_CHANNEL_KEYS.length;
  // 16-bit Terrain Total Size (2 bytes per cell)
  const terrainByteSize = resSq * 2 * TILE_TERRAIN_KEYS.length;
  const totalSize = channelsByteSize + terrainByteSize;

  // 1. 🎯 메모리 정렬을 보장하는 단일 ArrayBuffer 생성
  const buffer = new ArrayBuffer(totalSize);
  const combined8 = new Uint8Array(buffer);

  let offset = 0;

  // 2. Pack 8-bit Channels (Water -> Base -> Transport -> Structure)
  for (const key of TILE_CHANNEL_KEYS) {
    const data = channels[key];
    combined8.set(data, offset);
    offset += resSq;
  }

  // 3. Pack 16-bit Terrain (Height)
  for (const key of TILE_TERRAIN_KEYS) {
    const data = terrain[key]; // Uint16Array
    // 8-bit 뷰의 특정 위치를 16-bit 뷰로 해석하여 데이터 복사
    const target16 = new Uint16Array(buffer, offset, resSq);
    target16.set(data);

    offset += resSq * 2;
  }

  // 4. Compress (Level 9: CPU를 써서 트래픽 최소화)
  const compressed = gzipSync(combined8, { level: 9 });
  return (compressed as any).toString('base64');
};

// ----------------------------------------------------------------------
// 2. Unpacking Logic (Client/Internal 사용)
// ----------------------------------------------------------------------

export const unpackTileGrids = async (
  b64: string,
  res: number = TILE_RESOLUTION,
): Promise<TileGrids> => {
  const decompressed = await decompressBase64(b64);
  const resSq = res * res;
  let offset = 0;

  // 1. Unpack Channels
  const channels = {} as TileChannels;
  for (const key of TILE_CHANNEL_KEYS) {
    // Zero-copy slicing
    channels[key] = decompressed.subarray(offset, offset + resSq);
    offset += resSq;
  }

  // 2. Unpack Terrain
  const terrain = {} as TileTerrain;
  for (const key of TILE_TERRAIN_KEYS) {
    const byteLength = resSq * 2;

    // slice는 새로운 버퍼를 생성하므로 정렬 문제가 해결되지만,
    // 성능을 위해 subarray를 쓰려면 버퍼의 byteOffset이 2의 배수인지 확인해야 합니다.
    const sub = decompressed.slice(offset, offset + byteLength);

    // TypedArray 생성 시 버퍼를 직접 할당
    terrain[key] = new Uint16Array(sub.buffer, sub.byteOffset, resSq);

    offset += byteLength;
  }

  return {
    channels,
    terrain,
  };
};
