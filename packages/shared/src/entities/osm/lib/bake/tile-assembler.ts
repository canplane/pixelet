/**
 * @file tile-assembler.ts
 * 정규화된 OsmFeature를 타일 그리드와 POI 엔티티로 조립합니다.
 */

import { bakeTileGrid } from './tile-grid-baker';
import type { OsmFeature, MapPoi } from '../../model/osm-feature';
import type { GridLayers } from '../../model/tiles';
import type { TileCoord } from '../utils/tile-utils';

/**
 * [assembleTile]
 * 피처 목록을 받아 그리드 레이어와 POI 리스트를 한꺼번에 생성합니다.
 */
export function assembleTile(
  coord: TileCoord,
  elements: OsmFeature[],
  options?: { tileSize: number; cellSize: number },
) {
  // 1. POI 추출 (내부 함수 호출)
  const pois = extractPois(elements);

  // 2. 그리드 베이킹 (tile-grid-baker 호출)
  let partialLayers: Partial<GridLayers> = {};
  if (elements.length > 0) {
    partialLayers = bakeTileGrid(coord, elements);
  }

  return { pois, partialLayers };
}

/**
 * [extractPois]
 * 지도에 표시할 중요한 장소(POI)들을 선별합니다.
 */
export const extractPois = (features: OsmFeature[]): MapPoi[] => {
  const pois: MapPoi[] = [];

  for (const feature of features) {
    const { tags, type, category, center, extra, id } = feature;

    // [Filter Policy]
    // 1. 카테고리가 'poi'로 분류되었거나
    // 2. Node 타입이면서 이름이 있거나
    // 3. Institutional/Sports 시설이면서 이름이 있는 경우
    const isExplicitPoi = category === 'poi';
    const isNamedNode = type === 'node' && !!tags.name;
    const isNamedFacility =
      !!tags.name && (extra.isInstitutional || extra.isSports);

    if ((isExplicitPoi || isNamedNode || isNamedFacility) && center) {
      pois.push({
        id: `poi-${id}`,
        type: 'poi',
        // 식별 가능한 카테고리 우선 순위 (amenity > shop > tourism > category)
        category:
          tags.amenity || tags.shop || tags.tourism || category || 'place',
        name: tags.name || 'Unknown',
        center,
        tags,
      });
    }
  }

  return pois;
};
