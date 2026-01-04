/**
 * @file tile-terrain-packer.ts
 * 스키마 정의(tiles.ts)에 따라 동적으로 격자 데이터를 직렬화합니다.
 */

import { OSM_CONSTANTS } from '@pixelet/core/entities/osm/model/osm-constants';
import {
  TILE_CHANNEL_KEYS,
  type TileTerrain,
  type TileChannels,
} from '../model/tiles';
import { decompressBase64 } from './compression-utils';

const { TILE_RES } = OSM_CONSTANTS;

/**
 * [packTileTerrain]
 * Channels(8-bit)와 Elevation(16-bit)을 단일 바이너리로 직렬화합니다.
 */
export const packTileTerrain = (
  terrain: TileTerrain,
  gzipSync: (buf: Uint8Array, opt?: any) => any,
): string => {
  const { channels, elevation } = terrain;

  // [Safety] water 채널 등이 누락되었을 경우를 대비해 channels.base 기준으로 해상도 산출
  const resSq = channels.base.length;

  // 1. 전체 사이즈 계산 (Channels * 4 + Elevation * 2 bytes)
  const totalSize = resSq * TILE_CHANNEL_KEYS.length + resSq * 2;

  // 메모리 정렬을 보장하는 단일 ArrayBuffer 생성
  const buffer = new ArrayBuffer(totalSize);
  const u8View = new Uint8Array(buffer);

  let offset = 0;

  // 2. Pack 8-bit Channels
  for (const key of TILE_CHANNEL_KEYS) {
    u8View.set(channels[key], offset);
    offset += resSq;
  }

  // 3. Pack 16-bit Elevation (Byte order: Little Endian 기준)
  const u16view = new Uint16Array(buffer, offset, resSq);
  u16view.set(elevation);

  // 4. Compress & B64 (Level 9: CPU를 써서 트래픽 최소화)
  const compressed = gzipSync(u8View, { level: 9 });
  return (compressed as any).toString('base64');
};

/**
 * [unpackTileTerrain]
 * 바이너리 데이터를 다시 TileTerrain 구조로 복원합니다.
 * 🚀 Zero-Copy: .slice() 없이 원본 버퍼의 View를 그대로 사용합니다.
 */
export const unpackTileTerrain = async (
  b64: string,
  res: number = TILE_RES,
): Promise<TileTerrain> => {
  const decompressed = await decompressBase64(b64);
  const resSq = res * res;
  let offset = 0;

  // 1. Unpack Channels (Zero-copy Subarray)
  const channels = {} as TileChannels;
  for (const key of TILE_CHANNEL_KEYS) {
    // subarray는 새로운 버퍼 생성 없이 View만 반환합니다.
    channels[key] = decompressed.subarray(offset, offset + resSq);
    offset += resSq;
  }

  // 2. Unpack Elevation (Zero-Copy View)
  // 🎯 Elevation(16-bit)은 2바이트 정렬이 필수입니다.
  // TILE_RES가 짝수(예: 256)라면 offset도 짝수이므로,
  // decompressed.byteOffset이 짝수라는 전제하에 안전하게 View를 생성할 수 있습니다.
  const elevation = new Uint16Array(
    decompressed.buffer, // 원본 ArrayBuffer 참조 (공유)
    decompressed.byteOffset + offset, // 실제 메모리 시작 위치
    resSq,
  );

  /**
   * [Engineering Note]
   * 이제 channels와 elevation은 하나의 거대한 ArrayBuffer를 공유합니다.
   * Worker에서 전송 시 `Set`을 통해 중복을 제거하므로,
   * 1MB짜리 타일이라도 복사 비용 0, 전송 비용 1회로 처리됩니다.
   */
  return { channels, elevation };
};
