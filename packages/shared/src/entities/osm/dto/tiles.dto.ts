/**
 * @file tiles.dto.ts
 * 서버-클라이언트 간의 타일 데이터 전송 규격을 정의합니다.
 */

import type { TileCoord } from '../lib/utils/tile-utils';

/**
 * [GeometryTileDto]
 * 바이너리 격자 데이터가 Gzip + Base64로 인코딩된 전송 객체입니다.
 */
export interface GeometryTileDto {
  coord: TileCoord;
  grids: string; // 압축
  pois: string; // 압축
}
