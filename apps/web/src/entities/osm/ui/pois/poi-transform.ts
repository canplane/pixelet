import { type Coord2D } from '@pixelet/shared/shared/model/coords';
import type { GeometryTile } from '@pixelet/shared/entities/osm/model/tiles';
import { OSM_CONSTANTS } from '@pixelet/shared/entities/osm/model/osm-constants';
import {
  getTileCoord,
  getTileKey,
  getTileLocalGridIdx,
} from '@pixelet/shared/entities/osm/lib/utils/tile-utils';

const { CELL_SIZE, Z_SCALE_RATIO, TILE_RESOLUTION } = OSM_CONSTANTS;

/**
 * [getTerrainAltitude]
 * 절대 좌표(AbsPos)를 입력받아 해당 위치의 지형 높이(Meter)를 반환합니다.
 * O(1) Lookup으로 매우 빠릅니다.
 *
 * @param absPos 절대 미터 좌표
 * @param tiles 타일 데이터 Map (Store에서 가져옴)
 */
export function getTerrainAltitude(
  absPos: Coord2D,
  tiles: Map<string, GeometryTile>,
): number {
  // 1. 타일 찾기
  const coord = getTileCoord(absPos);
  const key = getTileKey(coord);
  const tile = tiles.get(key);

  if (!tile) return 0; // 데이터 없으면 해수면(0m) 처리

  // 3. 그리드 인덱스 변환 (새로 만든 tile-utils 활용)
  const { gx, gy } = getTileLocalGridIdx(absPos, coord);

  // 4. 범위 방어 코드 (Boundary Check)
  if (gx < 0 || gx >= TILE_RESOLUTION || gy < 0 || gy >= TILE_RESOLUTION)
    return 0;

  // 5. 높이 샘플링 및 스케일 적용
  const index = gy * TILE_RESOLUTION + gx;
  return (tile.grids.terrain.elevation[index] || 0) * CELL_SIZE * Z_SCALE_RATIO;
}
