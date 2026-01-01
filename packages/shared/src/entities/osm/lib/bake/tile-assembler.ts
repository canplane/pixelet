/**
 * @file tile-assembler.ts
 * 정규화된 OsmFeature를 타일 그리드와 POI 엔티티로 조립합니다.
 */

import { bakeTileChannels } from './tile-channel-baker';
import type { OsmFeature, MapPoi } from '../../model/osm-feature';
import type { TileCoord } from '../utils/tile-utils';

/**
 * [TileBuildConfig]
 * 타일 조립 과정에서 데이터 소스 및 환경에 따른 제어 옵션을 정의합니다.
 */
export interface TileBuildConfig {
  /**
   * 📉 [Limited Data Mode]
   * - true: 해안선 등 대규모 폴리곤 데이터가 제한적인 상황.
   * 전체 바다 채우기(Inverse Rendering)를 비활성화하여 내륙 침수를 방지합니다.
   * - false/undefined: 모든 데이터셋이 구비된 상황.
   * 정밀한 해안선 표현을 위해 전체 바다 채우기를 활성화합니다.
   */
  limited?: boolean;
}

/**
 * [assembleTile]
 * 피처 목록을 받아 타일 채널과 POI 리스트를 한꺼번에 생성합니다.
 * 🌊 oceanFeatures가 포함된 elements를 받아 래스터화 과정을 조율합니다.
 */
export function assembleTile(
  coord: TileCoord,
  elements: OsmFeature[],
  options?: TileBuildConfig,
) {
  // 1. POI 추출 (바다 제외 로직 포함됨)
  const pois = extractPois(elements);

  // 2. 그리드 베이킹
  // 데이터가 없더라도 GridPacker가 기대하는 5개 채널 구조를 유지하기 위해
  // 기본적으로 EmptyChannels를 기반으로 시작하는 것이 안전합니다.
  const partialChannels = bakeTileChannels(coord, elements, options);

  return { pois, partialChannels };
}

/**
 * [extractPois]
 * 지도에 표시할 중요한 장소(POI)들을 선별합니다.
 */
export const extractPois = (features: OsmFeature[]): MapPoi[] => {
  const pois: MapPoi[] = [];

  for (const feature of features) {
    const { tags, type, category, center, extra, id } = feature;
    const { name } = tags;

    // 🌊 [Guard] 바다는 절대 POI가 될 수 없음 + 필수 데이터 검증
    if (extra.isOcean || !center || !name) continue;

    // 🎯 [Refine] 수집 정책 보강
    const isExplicitPoi = category === 'poi';
    const isMajorFacility = extra.isInstitutional || extra.isSports;

    // [Named Node] 단순히 모든 노드를 가져오지 않고,
    // 최소한의 장소 정보(amenity, shop, tourism 등)가 있는 노드만 선별
    const isNamedNode =
      type === 'node' &&
      (!!tags.amenity ||
        !!tags.shop ||
        !!tags.tourism ||
        !!tags.place ||
        !!tags.historic);

    // [Policy Check]
    if (isExplicitPoi || isMajorFacility || isNamedNode) {
      pois.push({
        id: `poi-${id}`,
        type: 'poi',
        // 우선순위: amenity > shop > tourism > category
        category:
          tags.amenity || tags.shop || tags.tourism || category || 'place',
        name,
        center,
        rank: extra.rank, // Normalizer에서 이미 주입됨
        tags,
      });
    }
  }

  // 📈 [Density Control] 랭크 순 정렬 (클라이언트 렌더링 최적화 힌트)
  return pois.sort((a, b) => b.rank - a.rank);
};
