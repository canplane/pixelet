/**
 * @file osm-parser.ts
 * 정규화된 OsmFeature를 엔진에서 사용 가능한 인프라 및 POI 객체로 최종 변환합니다.
 * 레이어 간의 고도(Y-Offset)를 적용하여 Z-Fighting을 방지합니다.
 */

import type { OsmFeature, MapInfra, MapPoi } from '../model/osm-feature';

/**
 * 레이어 렌더링 우선순위 (Baking 순서 결정)
 * 숫자가 낮을수록 밑바닥(Base), 높을수록 최상단(Structure)
 */
const LAYER_PRIORITY: Record<string, number> = {
  landuse: 1,
  forest: 1, // forest도 landuse 레벨
  water: 2,
  road: 3,
  building: 4,
} as const;

export const parseOsmData = (
  elements: OsmFeature[],
): { infras: MapInfra[]; pois: MapPoi[] } => {
  const result = { infras: [] as MapInfra[], pois: [] as MapPoi[] };

  elements.forEach((item) => {
    const { tags, type, shape, computed, center, category } = item;

    // [SECTION 1] POI 처리 (Node 기반 장소 정보)
    // GeoService에서 'poi'로 분류했거나 Node 타입이면서 이름이 있는 경우
    if ((category === 'poi' || type === 'node') && tags.name && center) {
      result.pois.push({
        id: `poi-${item.id}`,
        type: 'poi',
        category: tags.amenity || tags.shop || tags.tourism || 'place',
        name: tags.name,
        center,
        tags,
      });
      // 카테고리가 명확히 POI인 경우 여기서 처리를 마칠 수 있음
      if (category === 'poi') return;
    }

    // [SECTION 2] 인프라 처리 (Way & Relation 기반 지형/구조물)
    if (shape && shape.points.length >= 2) {
      // GeoService에서 확정한 category를 infraType으로 그대로 사용
      // 값이 없으면 가장 낮은 우선순위인 'landuse'로 Fallback
      const infraType = category || 'landuse';

      // --- [Y-Offset Stacking & Z-Fighting Defense] 원본 오프셋 로직 계승 ---
      const offsetZ = calculateLayerOffset(infraType, tags, computed.layer);
      const pointsWithOffset = shape.points.map((p) => ({
        ...p,
        z: p.z + offsetZ,
      }));

      // 내부 플래그 주입 (Baking 엔진 대응)
      if (infraType === 'road')
        tags._calculatedWidth = computed.width.toString();

      result.infras.push({
        id: `infra-${item.id}`,
        type: infraType,
        points: pointsWithOffset,
        center,
        shapePath: {
          points: pointsWithOffset,
          isClosed: shape.isClosed,
          holes: shape.holes,
        },
        height: computed.height,
        tags,
      });
    }
  });

  // 레이어 우선순위에 따른 최종 정렬
  result.infras.sort(
    (a, b) => (LAYER_PRIORITY[a.type] || 0) - (LAYER_PRIORITY[b.type] || 0),
  );

  return result;
};

/**
 * 원본 코드의 레이어 오프셋 계산 로직을 분리하여 유지합니다.
 */
function calculateLayerOffset(
  type: string,
  tags: any,
  layerLevel: number,
): number {
  if (type === 'road') {
    let offset = 0.05 + layerLevel * 0.2;
    if (tags.bridge === 'yes') offset += 1.0;
    if (tags.tunnel === 'yes') offset -= 1.0;
    return offset;
  }
  if (type === 'water') return -0.15 + layerLevel * 0.1;
  if (type === 'forest') return 0.022; // 일반 landuse(0.02)보다 살짝 높게 설정
  if (type === 'landuse') {
    if (tags._isInstitutional) return 0.025;
    if (tags._isParking) return 0.03;
    if (tags._isSports) return 0.035;
    return 0.02;
  }
  return 0;
}
