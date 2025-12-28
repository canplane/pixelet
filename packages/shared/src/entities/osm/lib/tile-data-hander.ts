import { parseOsmData } from './osm-parser';
import { bakeTileGrid } from './tile-baker';
import type { MeterPos } from '@pixelet/shared/shared/model/coords';
import type { OverpassElement } from '../api/overpass-types';
import type { MapInfra, MapPoi } from '../model/osms';
import type { TileGrids } from '../model/tiles';

/**
 * [process-osm-to-grids]
 * Raw OSM 데이터를 인프라 객체와 그리드 데이터로 변환합니다.
 * 이 함수는 특정 타일 상태를 변경하지 않는 순수 함수입니다.
 */
export function processOsmToGrids(
  elements: OverpassElement[],
  origin: MeterPos,
  config: { tileSize: number; cellSize: number },
): {
  infras: MapInfra[];
  pois: MapPoi[];
  partialGrids: Partial<TileGrids>;
} {
  /**
   * (A) 파싱: 'osm-parser' 실행
   * [보강] 'elements'에는 (._; > ;) 쿼리 결과로 포함된 다량의 순수 Node 좌표 데이터가 섞여 있습니다.
   * parseOsmData 내부의 nodeCache가 이 elements를 전체 순회하여 Way의 좌표를 100% 복구합니다.
   */
  const { infras, pois } = parseOsmData(elements);

  // (B) 그리드 베이킹 (인프라가 있을 때만 실행)
  let partialGrids: Partial<TileGrids> = {};
  if (infras.length > 0) {
    const result = bakeTileGrid(infras, origin, config);
    partialGrids = result.grids;
  }

  return { infras, pois, partialGrids };
}
