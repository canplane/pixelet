/**
 * @file grid-binary.ts
 * OSM 그리드 데이터의 규격 정의 및 바이너리 인코딩 로직을 담당합니다.
 * 이 파일은 순수 함수로 구성되어 서버와 클라이언트에서 공통으로 참조 가능합니다.
 */

import { OSM_CONSTANTS } from '@pixelet/core/entities/osm/model/osm-constants';

const { TILE_RES: res } = OSM_CONSTANTS;

/**
 * [generateGridBinary]
 * 그리드 원본 데이터(Uint8Array)에 표준 헤더를 부착하여 바이너리 버퍼를 생성합니다.
 *
 * * [Header Specification] (Total 12 Bytes, Little Endian)
 * - 0~3 byte: Resolution (int32)
 * - 4~7 byte: Origin X (int32) -> 0
 * - 8~11 byte: Origin Y (int32) -> 0 (South Anchor)
 *
 * * 💡 [Data Order Note]
 * 부착되는 grid 데이터는 South-to-North(Bottom-Up) 순서로 정렬되어 있습니다.
 *
 * @param grid - South-to-North 순서의 원본 데이터
 * @param tileOriginX - 타일 내 로컬 시작 X (기본값 0)
 * @param tileOriginY - 타일 내 로컬 시작 Y (기본값 0, South Anchor)
 */
export function generateGridBinary(
  grid: Uint8Array,
  tileOriginX: number = 0,
  tileOriginY: number = 0,
): Uint8Array {
  const HEADER_SIZE = 12;
  const combinedBuffer = new Uint8Array(HEADER_SIZE + grid.length);
  const view = new DataView(combinedBuffer.buffer);

  // 🎯 Header Spec (Little Endian): Resolution, OriginX(0), OriginY(0)
  view.setInt32(0, res, true);
  view.setInt32(4, tileOriginX, true);
  view.setInt32(8, tileOriginY, true);

  combinedBuffer.set(grid, HEADER_SIZE);
  return combinedBuffer;
}
