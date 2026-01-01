/**
 * @file grid-data.ts
 * OSM 그리드 데이터의 규격 정의 및 바이너리 인코딩 로직을 담당합니다.
 * 이 파일은 순수 함수로 구성되어 서버와 클라이언트에서 공통으로 참조 가능합니다.
 */

import { OSM_CONSTANTS } from '@pixelet/shared/entities/osm/model/osm-constants';

const { TILE_RESOLUTION: resolution } = OSM_CONSTANTS;

/**
 * [generateGridBinary]
 * 그리드 원본 데이터(Uint8Array)에 표준 헤더를 부착하여 바이너리 버퍼를 생성합니다.
 *
 * * [Header Specification] (Total 12 Bytes, Little Endian)
 * - 0~3 byte: Resolution (int32)
 * - 4~7 byte: Origin X (int32) -> 0
 * - 8~11 byte: Origin Y (int32) -> 0
 */
export function generateGridBinary(grid: Uint8Array): Uint8Array {
  const HEADER_SIZE = 12;
  const combinedBuffer = new Uint8Array(HEADER_SIZE + grid.length);
  const view = new DataView(combinedBuffer.buffer);

  // 🎯 Header Spec (Little Endian): Resolution, OriginX(0), OriginY(0)
  view.setInt32(0, resolution, true);
  view.setInt32(4, 0, true);
  view.setInt32(8, 0, true);

  combinedBuffer.set(grid, HEADER_SIZE);
  return combinedBuffer;
}
