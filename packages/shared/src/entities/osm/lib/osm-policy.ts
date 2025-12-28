/**
 * @file osm-policy.ts
 * GeoService(DB)와 OsmNormalizer(API)가 공유하는 OSM 태그 해석 정책입니다.
 * 분류(Category), 강제 폐곡(ForceClosed), 물리 수치(Metrics) 계산의 단일 진실 공급원(SSOT)입니다.
 */

// 🎯 도메인 지식 매핑 Import
import { ROAD_THICKNESS_MAP } from './osm-mapping';

export const determineOsmCategory = (
  tags: Record<string, string | undefined>,
): string => {
  // 1. STRUCTURE -> 'building'
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
  // 2. WATER -> 'water'
  if (
    tags.waterway ||
    tags.water ||
    ['water', 'cliff', 'coastline'].includes(tags.natural || '') ||
    tags.landuse === 'reservoir'
  ) {
    return 'water';
  }
  // 3. POI -> 'poi'
  if (
    tags.amenity ||
    tags.shop ||
    tags.tourism ||
    tags.historic ||
    ['station', 'subway_entrance'].includes(tags.railway || '')
  ) {
    return 'poi';
  }
  // 4. TRANSPORT -> 'road'
  if (
    tags.highway ||
    tags.railway ||
    tags.aeroway ||
    tags['area:highway'] ||
    tags.junction
  ) {
    return 'road';
  }
  // 5. NATURAL / FOREST -> 'forest'
  if (
    tags.natural === 'wood' ||
    tags.natural === 'peak' ||
    tags.natural === 'mountain' ||
    tags.natural === 'scrub' ||
    tags.natural === 'fell' ||
    tags.natural === 'bare_rock' ||
    tags.landuse === 'forest' ||
    tags.leisure === 'nature_reserve'
  ) {
    return 'forest';
  }
  // 6. Generic Landuse -> 'landuse' (Default)
  if (
    tags.landuse === 'grass' ||
    tags.landuse === 'meadow' ||
    tags.leisure === 'park'
  ) {
    return 'landuse';
  }

  return 'landuse';
};

/**
 * 기하학적으로 열려 있어도 태그 속성상 면(Polygon)으로 처리해야 하는지 판별합니다.
 */
export const shouldForceClosed = (
  tags: Record<string, string | undefined>,
): boolean => {
  return (
    tags.amenity === 'parking' ||
    ['school', 'university', 'hospital'].includes(tags.amenity || '') ||
    ['pitch', 'stadium', 'park', 'garden'].includes(tags.leisure || '') ||
    !!tags.landuse
  );
};

/**
 * 태그를 기반으로 물리적 수치(높이, 폭, 레이어)를 계산합니다.
 * 🚀 보강: lanes 태그 누락 시 ROAD_THICKNESS_MAP을 참조하여 기본 폭을 할당합니다.
 */
export const computeOsmMetrics = (
  tags: Record<string, string | undefined>,
  category: string,
): { height: number; width: number; layer: number } => {
  // --- [높이 계산] (기존 로직 유지) ---
  const levels = parseFloat(tags['building:levels'] || '1');
  const minLevel = parseFloat(tags['building:min_level'] || '0');
  const roofHeight = parseFloat(tags['roof:height'] || '0');

  const rawHeight =
    category === 'building'
      ? tags.height
        ? parseFloat(tags.height)
        : (levels - minLevel) * 3.5 + roofHeight
      : 0;

  // --- [폭 계산] (로직 보강) ---
  let lanes = 1; // 최후의 기본값 (기존과 동일)

  if (tags.lanes) {
    // 1순위: 명시적 차선 태그가 있으면 사용 (기존 로직)
    lanes = parseInt(tags.lanes, 10);
  } else {
    // 2순위: 태그가 없을 때만 도메인 지식(Map) 참조 (신규 보강)
    // highway가 없으면 railway 확인
    const typeKey = tags.highway || (tags.railway ? 'rail' : 'unclassified');
    lanes = ROAD_THICKNESS_MAP[typeKey] || 1;
  }

  const widthTag = tags.width ? parseFloat(tags.width) : 0;

  // width 태그가 최우선, 없으면 차선 * 3.5m (기존 로직 유지)
  const rawWidth =
    category === 'road' ? (widthTag > 0 ? widthTag : lanes * 3.5) : 0;

  // --- [레이어 계산] (기존 로직 유지) ---
  const rawLayer = parseInt(tags.layer || '0', 10);

  return {
    height: isNaN(rawHeight)
      ? category === 'building'
        ? 3.5
        : 0
      : Math.max(0, rawHeight),
    width: isNaN(rawWidth) ? 0 : rawWidth,
    layer: isNaN(rawLayer) ? 0 : rawLayer,
  };
};
