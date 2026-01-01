import { type Coord2D } from '@pixelet/shared/shared/model/coords';
import type { Tile } from '@pixelet/shared/entities/osm/model/tiles';
import { OSM_CONSTANTS } from '@pixelet/shared/entities/osm/model/osm-constants';
import {
  getTileCoord,
  getTileKey,
  absPosToTileOffset,
  tileOffsetToCellIdx,
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
  tiles: Map<string, Tile>,
): number {
  // 1. 타일 찾기
  const coord = getTileCoord(absPos);
  const key = getTileKey(coord);
  const tile = tiles.get(key);

  if (!tile) return 0; // 데이터 없으면 해수면(0m) 처리

  // 3. 셀 인덱스 변환
  const { cx, cy } = tileOffsetToCellIdx(absPosToTileOffset(absPos));

  // 4. 범위 방어 코드 (Boundary Check)
  if (cx < 0 || cx >= TILE_RESOLUTION || cy < 0 || cy >= TILE_RESOLUTION)
    return 0;

  // 5. 높이 샘플링 및 스케일 적용
  const index = cy * TILE_RESOLUTION + cx;
  return (tile.grids.terrain.elevation[index] || 0) * CELL_SIZE * Z_SCALE_RATIO;
}
