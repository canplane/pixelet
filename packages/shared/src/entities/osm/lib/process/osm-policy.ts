/**
 * @file osm-policy.ts
 * [Single Source of Truth]
 * OSM 원시 데이터(Tags)를 우리 시스템의 규격(Category, Metrics, Flags)으로 해석하는 단일 정책 파일입니다.
 *
 * * [설계 의도]
 * 1. SSOT: 매핑 테이블(Data)과 해석 로직(Policy)을 한곳에 통합하여 도메인 지식의 분산을 막습니다.
 * 2. Performance: 타일 베이킹 루프(수만 번 반복) 성능을 위해 조회용 상수는 Set/Map(O(1))을 사용합니다.
 * 3. Fallback: 태그가 누락된 경우에도 적절한 기본값을 할당하여 시각적 오류를 방지합니다.
 */

import type { OsmTags } from '@pixelet/shared/entities/osm/model/osm-source';
import type { OsmExtra } from '@pixelet/shared/entities/osm/model/osm-feature';

// =============================================================================
// SECTION 1. [Domain Knowledge] 해석 기준 데이터 (Internal Lookups)
// * 외부 노출 없이 정책 로직 내부에서만 참조되는 '해석용 상수'들입니다.
// =============================================================================

/**
 * [Road Thickness Map]
 * 도로 종류(highway)별 기본 차선 수입니다.
 * lanes 태그가 없는 경우, 이 값을 참조하여 도로의 시각적 위계를 표현합니다.
 */
const DEFAULT_LANES_BY_HIGHWAY: Record<string, number> = {
  motorway: 4, // 고속도로 (가장 넓음)
  trunk: 3, // 간선도로
  primary: 3, // 주요 도로
  secondary: 2, // 보조 도로
  tertiary: 2, // 지역 도로
  residential: 1, // 주거지 도로
  service: 1, // 서비스 도로
  unclassified: 1, // 미분류
  path: 0.5, // 오솔길 (사람만 다님)
  footway: 0.5, // 보도
  rail: 1, // 철도
};

/**
 * [Institutional Check]
 * 대형 공공기관이나 교육/의료 시설로 분류할 amenity 태그 목록입니다.
 * (Baker 단계에서 특수 텍스처 매핑에 활용됨)
 */
const INSTITUTIONAL_TYPES = new Set([
  'school',
  'university',
  'college',
  'hospital',
  'public_building',
  'civic_centre',
  'government',
]);

/**
 * [Sports Check]
 * 스포츠 시설로 분류할 leisure/sport 관련 태그 목록입니다.
 */
const SPORTS_TYPES = new Set([
  'pitch',
  'stadium',
  'track',
  'golf_course',
  'sports_centre',
  'playground',
]);

/**
 * [Force Close Rules]
 * 선형(LineString) 데이터로 오더라도 면(Polygon)으로 닫아서 처리해야 할 시설들입니다.
 */
const FORCE_CLOSE_AMENITIES = new Set([
  'parking',
  'school',
  'university',
  'hospital',
]);
const FORCE_CLOSE_LEISURE = new Set(['pitch', 'stadium', 'park', 'garden']);

// =============================================================================
// SECTION 2. [Policy Logic] 데이터 해석 로직 (Exports)
// * 위 상수들을 활용하여 실제 값을 계산하거나 판단하는 순수 함수들입니다.
// =============================================================================

/**
 * [Category Determination]
 * 태그의 우선순위(Priority)에 따라 지형지물의 타입을 결정합니다.
 *
 * * [Priority Order]
 * 1. Structure (건물, 구조물) - 가장 중요
 * 2. Water (수계) - 지형의 기초
 * 3. POI (장소) - 랜드마크
 * 4. Road (교통) - 네트워크
 * 5. Nature (자연) - 배경
 */
export const determineOsmCategory = (tags: OsmTags): string => {
  // 1. STRUCTURE
  if (
    tags.building ||
    tags['building:part'] ||
    tags.man_made === 'pier' ||
    tags.man_made ||
    tags.bridge === 'yes' ||
    tags.tunnel === 'yes'
  ) {
    return 'building';
  }

  // 2. WATER
  if (
    tags.waterway ||
    tags.water ||
    ['water', 'cliff', 'coastline'].includes(tags.natural || '') ||
    tags.landuse === 'reservoir'
  ) {
    return 'water';
  }

  // 3. POI
  if (
    tags.amenity ||
    tags.shop ||
    tags.tourism ||
    tags.historic ||
    ['station', 'subway_entrance'].includes(tags.railway || '')
  ) {
    return 'poi';
  }

  // 4. TRANSPORT
  if (
    tags.highway ||
    tags.railway ||
    tags.aeroway ||
    tags['area:highway'] ||
    tags.junction
  ) {
    return 'road';
  }

  // 5. NATURAL / FOREST
  if (
    tags.landuse === 'forest' ||
    tags.leisure === 'nature_reserve' ||
    ['wood', 'peak', 'mountain', 'scrub', 'fell', 'bare_rock'].includes(
      tags.natural || '',
    )
  ) {
    return 'forest';
  }

  // 6. GENERIC (Default)
  return 'landuse';
};

/**
 * [Metric Calculation]
 * 태그와 매핑 테이블을 조합하여 물리적 수치(높이, 폭, 레이어)를 계산합니다.
 *
 * * [Logic: Width]
 * 1. width 태그 존재 시 최우선 사용
 * 2. lanes 태그 존재 시 lanes * 3.5m 적용
 * 3. 둘 다 없으면 highway 타입에 따른 기본 차선 수(DEFAULT_LANES_BY_HIGHWAY) * 3.5m 적용
 */
export const computeOsmMetrics = (
  tags: OsmTags,
  category: string,
): { height: number; width: number; layer: number } => {
  // --- 1. Height Calculation ---
  let height = 0;
  if (category === 'building') {
    if (tags.height) {
      height = parseFloat(tags.height);
    } else {
      // 층수 기반 추정 (1층당 3.5m + 지붕)
      const levels = parseFloat(tags['building:levels'] || '1');
      const minLevel = parseFloat(tags['building:min_level'] || '0');
      const roofHeight = parseFloat(tags['roof:height'] || '0');
      height = (levels - minLevel) * 3.5 + roofHeight;
    }
  }

  // --- 2. Width Calculation ---
  let width = 0;
  if (category === 'road') {
    if (tags.width) {
      width = parseFloat(tags.width);
    } else {
      // Fallback Strategy: Lanes Tag -> Mapping Table -> Default(1)
      let lanes = 1;
      if (tags.lanes) {
        lanes = parseInt(tags.lanes, 10);
      } else {
        const typeKey =
          tags.highway || (tags.railway ? 'rail' : 'unclassified');
        lanes = DEFAULT_LANES_BY_HIGHWAY[typeKey] || 1;
      }
      width = lanes * 3.5;
    }
  }

  // --- 3. Layer Calculation ---
  const layer = parseInt(tags.layer || '0', 10);

  // NaN 방지 및 음수 높이 방어
  return {
    height: Math.max(0, isNaN(height) ? 0 : height),
    width: isNaN(width) ? 0 : width,
    layer: isNaN(layer) ? 0 : layer,
  };
};

/**
 * [Flag Extraction]
 * 렌더링 단계(Baker)에서 복잡한 문자열 비교 없이 상태를 확인하기 위해
 * 불리언 플래그를 미리 추출합니다.
 */
export const computeOsmExtra = (tags: OsmTags, category: string): OsmExtra => {
  return {
    isParking:
      category === 'parking' ||
      tags.amenity === 'parking' ||
      tags.parking === 'surface',

    // Set.has()를 사용하여 O(1) 성능으로 체크
    isInstitutional: INSTITUTIONAL_TYPES.has(tags.amenity || ''),

    isSports: SPORTS_TYPES.has(tags.leisure || '') || !!tags.sport,
  };
};

/**
 * [Geometry Helper]
 * 기하학적으로 열려 있어도(LineString), 속성상 면(Polygon)으로 닫아서
 * 렌더링해야 하는지 여부를 판별합니다.
 */
export const shouldForceClosed = (tags: OsmTags): boolean => {
  return (
    !!tags.landuse ||
    FORCE_CLOSE_AMENITIES.has(tags.amenity || '') ||
    FORCE_CLOSE_LEISURE.has(tags.leisure || '')
  );
};
