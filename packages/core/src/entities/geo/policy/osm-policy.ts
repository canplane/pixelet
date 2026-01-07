/**
 * @file osm-policy.ts
 * 🧠 [Single Source of Truth]
 * OSM 원시 데이터(Tags)를 우리 시스템의 규격(Category, Metrics, Flags)으로 해석하는 단일 정책 파일입니다.
 *
 * * [설계 의도]
 * 1. 🎯 SSOT: 매핑 테이블(Data)과 해석 로직(Policy)을 한곳에 통합하여 도메인 지식의 분산을 막습니다.
 * 2. ⚡ Performance: 타일 베이킹 루프(수만 번 반복) 성능을 위해 조회용 상수는 Set/Map(O(1))을 사용합니다.
 * 3. 🛡️ Fallback: 태그가 누락된 경우에도 적절한 기본값을 할당하여 시각적 오류를 방지합니다.
 */

import type { OsmTags } from '@pixelet/core/entities/geo/model/osm-source';
import type { OsmExtra } from '@pixelet/core/entities/geo/model/osm-feature';
import { BASE_TYPES } from '@pixelet/core/entities/cell/model/base';
import { TileSurfaceKey } from '@pixelet/core/entities/geo/model/tiles';

// =============================================================================
// SECTION 1. [Domain Knowledge] 📚 해석 기준 데이터
// * 외부 노출 없이 정책 로직 내부에서만 참조되는 '해석용 상수'들입니다.
// =============================================================================

/**
 * 🛣️ [Road Thickness Map]
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
 * ✈️ [Aeroway Width Map]
 * 공항 시설물의 기본 폭(m)입니다.
 * width 태그가 없을 때 이 값을 사용하여 웅장함을 표현합니다.
 */
const DEFAULT_AEROWAY_WIDTHS: Record<string, number> = {
  runway: 45, // 국제공항급 활주로 (보통 45~60m)
  taxiway: 23, // 대형 항공기 유도로 (보통 20~25m)
  taxilane: 15, // 계류장 내 이동로
  apron: 0, // 면(Area)이므로 선폭 불필요 (보통 Polygon으로 옴)
};

/**
 * 🏫 [Institutional Check]
 * 대형 공공기관이나 교육/의료 시설로 분류할 amenity 태그 목록입니다.
 * (Baker 단계에서 특수 텍스처 매핑에 활용됨)
 * * 🔥 [UPDATE] 'train_station', 'transportation' 추가 -> 역사가 일반 건물과 다르게 보이도록 함
 */
const INSTITUTIONAL_TYPES = new Set([
  'school',
  'university',
  'college',
  'hospital',
  'public_building',
  'civic_centre',
  'government',
  'train_station', // 🚅 기차역/전철역
  'transportation', // 🚌 터미널 등
]);

const EDU_TYPES = new Set(['school', 'university', 'college', 'kindergarten', 'classroom', 'dormitory']);
const MEDICAL_TYPES = new Set(['hospital', 'clinic']);

/**
 * 🏟️ [Sports Check]
 * 스포츠 시설로 분류할 leisure/sport 관련 태그 목록입니다.
 */
const SPORTS_TYPES = new Set(['pitch', 'stadium', 'track', 'golf_course', 'sports_centre', 'playground']);

/**
 * 🔒 [Force Close Rules]
 * 선형(LineString) 데이터로 오더라도 면(Polygon)으로 닫아서 처리해야 할 시설들입니다.
 */
const FORCE_CLOSE_AMENITIES = new Set(['parking', 'school', 'university', 'hospital']);
const FORCE_CLOSE_LEISURE = new Set(['pitch', 'stadium', 'park', 'garden']);

// =============================================================================
// SECTION 2. [Policy Logic] ⚙️ 데이터 해석 로직
// * 위 상수들을 활용하여 실제 값을 계산하거나 판단하는 순수 함수들입니다.
// =============================================================================

// =============================================================================
// [Helper] 도로 위계별 셀 타입 매핑
// =============================================================================
function determineRoad(highwayTag: string): number {
  switch (highwayTag) {
    // 🔴 Level 1: 고속도로 및 간선도로
    case 'motorway':
    case 'motorway_link':
    case 'trunk':
    case 'trunk_link':
      return BASE_TYPES.TRANSPORT.ROAD_MOTORWAY;

    // 🟠 Level 2: 주요 도로 (대로)
    case 'primary':
    case 'primary_link':
      return BASE_TYPES.TRANSPORT.ROAD_PRIMARY;

    // 🟡 Level 3: 보조 도로 (중로)
    case 'secondary':
    case 'secondary_link':
    case 'tertiary': // 3차 도로까지는 노란색 계열로 묶거나
    case 'tertiary_link': // 디자인에 따라 Default(White)로 보낼 수도 있음
      return BASE_TYPES.TRANSPORT.ROAD_SECONDARY;

    // ⚪ Level 4: 일반/주거지 도로 (기본값)
    case 'residential':
    case 'living_street':
    case 'unclassified':
    case 'service':
    default:
      return BASE_TYPES.TRANSPORT.ROAD_DEFAULT;
  }
}

// [Helper] 철도 종류별 셀 타입 매핑
function determineRailway(tags: OsmTags): number {
  const type = tags.railway;

  // 1. 지하철 및 경전철 (Subway, Light Rail)
  if (type === 'subway' || type === 'light_rail') {
    return BASE_TYPES.TRANSPORT.RAIL_SUBWAY;
  }

  // 2. 일반 철도 (Rail)
  if (type === 'rail') {
    // service 태그가 있으면 기지창이나 지선이므로 위계를 낮출 수도 있습니다.
    return BASE_TYPES.TRANSPORT.RAIL_MAIN;
  }

  // 3. 기타 (Tram, Monorail 등) - 기본값으로 대응
  return BASE_TYPES.TRANSPORT.RAIL_MAIN;
}

/**
 * 📂 [Category Determination]
 * 태그의 우선순위(Priority)에 따라 지형지물의 타입을 결정합니다.
 *
 * * [Priority Order] (Auto-Generated Channeling Order)
 * 1. 🏔️ Base (지형) - 바다, 땅, 자연, 구역
 * 2. 💧 Water (내륙 수계)
 * 3. 🏢 Structure (구조물) - 건물
 * 4. 🛣️ Transport (교통) - 도로망
 */
export const determineOsmCategory = (tags: OsmTags): string => {
  // 0. 🌊 OCEAN
  if (
    tags.natural === 'coastline' ||
    tags.natural === 'ocean' ||
    tags.natural === 'sea' ||
    tags.source === 'osm-coastline' // 🎯 추가
  ) {
    return 'sea';
  }

  // 🏢 STRUCTURE
  if (
    tags.building ||
    tags['building:part'] ||
    tags.man_made === 'pier' ||
    // man_made가 'tunnel'이거나 'bridge'인 경우, 건물 카테고리에서 제외
    // 이렇게 해야 다리가 Road로, 터널이 Road/Landuse로 자연스럽게 흘러갑니다.
    (tags.man_made && !['tunnel', 'bridge'].includes(tags.man_made))
  ) {
    return 'building';
  }

  // 💧 WATER (내륙 수계)
  if (tags.waterway || tags.water || tags.natural === 'water' || tags.wetland || ['reservoir', 'basin'].includes(tags.landuse || '')) {
    return 'water';
  }

  // 📍 POI
  if (tags.amenity || tags.shop || tags.tourism || tags.historic || ['station', 'subway_entrance'].includes(tags.railway || '')) {
    return 'poi';
  }

  // ✈️ AEROWAY
  if (tags.aeroway) {
    if (['apron', 'aerodrome', 'helipad'].includes(tags.aeroway)) {
      return 'landuse'; // 면(Area)으로 그려질 배경 요소 -> Base Channel
    }
    return 'road'; // 선(Line)으로 그려질 교통망 (Runway, Taxiway) -> Transport Channel
  }

  // 🛣️ TRANSPORT
  if (tags.highway || tags.railway || tags['area:highway'] || tags.junction) {
    return 'road';
  }

  // 🌲 NATURAL / FOREST
  if (
    tags.landuse === 'forest' ||
    tags.leisure === 'nature_reserve' ||
    ['wood', 'peak', 'mountain', 'scrub', 'fell', 'bare_rock', 'cliff', 'sand', 'beach'].includes(tags.natural || '')
  ) {
    return 'forest'; // 혹은 'nature'로 명칭 확장 가능
  }

  // 🗺️ GENERIC (Default)
  return 'landuse';
};

/**
 * 📐 [Metric Calculation]
 * 태그와 매핑 테이블을 조합하여 물리적 수치(높이, 폭, 레이어)를 계산합니다.
 *
 * * [Logic: Width]
 * 1. width 태그 존재 시 최우선 사용
 * 2. lanes 태그 존재 시 lanes * 3.5m 적용
 */
export const computeOsmMetrics = (tags: OsmTags, category: string): { height: number; width: number; layer: number } => {
  // --- 1. 📏 Height Calculation ---
  let height = 0;
  if (category === 'building') {
    if (tags.height) height = parseFloat(tags.height);
    else {
      // 층수 기반 추정 (1층당 3.5m + 지붕)
      const levels = parseFloat(tags['building:levels'] || '1');
      const minLevel = parseFloat(tags['building:min_level'] || '0');
      const roofHeight = parseFloat(tags['roof:height'] || '0');
      height = (levels - minLevel) * 3.5 + roofHeight;
    }
  } else if (category === 'sea') {
    // 바다는 해수면 기준이므로 높이는 0이지만,
    // 나중에 심도(Depth) 데이터를 쓸 경우 여기서 처리합니다.
    height = 0;
  }

  // --- 2. ↕️ Width Calculation ---
  // 1. width 태그 존재 시 최우선 사용
  // 2. lanes 태그 존재 시 lanes * 3.5m 적용
  // 3. 둘 다 없으면 highway 타입에 따른 기본 차선 수(DEFAULT_LANES_BY_HIGHWAY) * 3.5m 적용
  let width = 0;
  if (category === 'road') {
    if (tags.width) {
      width = parseFloat(tags.width);
    } else if (tags.aeroway) {
      // ✈️ 공항 전용 너비 정책 적용
      width = DEFAULT_AEROWAY_WIDTHS[tags.aeroway] || 30;
    } else {
      // 기존 도로 너비 정책
      // Fallback Strategy: Lanes Tag -> Mapping Table -> Default(1)
      let lanes = 1;
      if (tags.lanes) {
        lanes = parseInt(tags.lanes, 10);
      } else {
        const typeKey = tags.highway || (tags.railway ? 'rail' : 'unclassified');
        lanes = DEFAULT_LANES_BY_HIGHWAY[typeKey] || 1;
      }
      width = lanes * 3.5;
    }
  } else if (category === 'water' && tags.waterway) {
    // [수로 기본 폭 보정] 강(River)이나 운하의 경우 width 태그가 없으면
    // 최소한의 가시성을 위해 기본 폭(예: 3m)을 부여합니다.
    width = tags.width ? parseFloat(tags.width) : 3;
  }

  // --- 3. 🪜 Layer Calculation ---
  const baseLayer = category === 'sea' ? -5 : 0;
  const layer = parseInt(tags.layer || baseLayer.toString(), 10);

  return {
    height: Math.max(0, isNaN(height) ? 0 : height),
    width: Math.max(0, isNaN(width) ? 0 : width),
    layer: isNaN(layer) ? baseLayer : layer,
  };
};

/**
 * 🔗 [Geometry Helper]
 */
export const shouldForceClosed = (tags: OsmTags): boolean => {
  return (
    !!tags.landuse ||
    tags.natural === 'ocean' || // 🌊 해안선 데이터 강제 폐곡
    tags.natural === 'sea' ||
    FORCE_CLOSE_AMENITIES.has(tags.amenity || '') ||
    FORCE_CLOSE_LEISURE.has(tags.leisure || '')
  );
};

/**
 * 🚩 [Flag Extraction]
 * 렌더링 단계(Baker)에서 복잡한 문자열 비교 없이 상태를 확인하기 위해
 * 불리언 플래그와 중요도를 미리 추출합니다.
 */
export const computeOsmExtra = (tags: OsmTags, category: string): OsmExtra => {
  // 🔥 [FIX] 역(Station) 판별 로직 강화
  // amenity 태그뿐만 아니라, railway나 building 태그도 확인하여 공공시설로 분류
  const isInstitutional =
    INSTITUTIONAL_TYPES.has(tags.amenity || '') || tags.railway === 'station' || ['train_station', 'transportation'].includes(tags.building || '');

  const isOcean = tags.source === 'osm-coastline' || tags.natural === 'ocean' || tags.natural === 'sea' || category === 'sea';

  return {
    /** 🚀 [Policy] 정책 기반 중요도 산출 */
    rank: calculateOsmRank(category, tags, { isInstitutional, isOcean }),

    /** 🌊 [Flag] 진짜 바다 여부 */
    isOcean, // Baker가 이 값을 보고 Inverse Rendering(파내기)을 수행합니다.

    /** 🅿️ [Flag] 주차장 여부 */
    isParking: category === 'parking' || tags.amenity === 'parking' || tags.parking === 'surface',

    /** 🏛️ [Flag] 공공/교육 시설 여부 */
    isInstitutional,

    /** ⚽ [Flag] 스포츠 시설 여부 */
    isSports: SPORTS_TYPES.has(tags.leisure || '') || !!tags.sport,
  };
};

/**
 * 📊 [calculateOsmRank]
 * 엔진 내 렌더링 및 POI 노출 우선순위를 결정합니다.
 *
 * [Engineering Note]
 * 1. Layer Priority: 물리적 높이(Layer)는 무조건 최우선입니다. (가중치 100)
 * 2. Hierarchy Priority: 같은 층에서는 '도로 위계가 높은(넓은)' 도로가
 * '낮은(좁은)' 도로를 덮어써야(Draw Last) 도로망의 시각적 연속성이 보장됩니다.
 */
function calculateOsmRank(category: string, tags: OsmTags, flags: { isInstitutional: boolean; isOcean: boolean }): number {
  let score = 10; // Base score (Base Land가 보통 10점을 가짐)

  // 1. 🗂️ 카테고리별 가중치
  if (category === 'water') score = 5; // 5점 -> Base(10점)보다 먼저 그려짐

  if (category === 'poi') score += 50;
  if (category === 'building' && tags.name) score += 20;

  // 2. 🛣️ 도로 위계별 가중치
  if (category === 'road') {
    const h = tags.highway || '';

    // [Draw Order: Hierarchy]
    // 덜 중요한 도로(Residential)를 먼저 그리고(깔고),
    // 중요한 도로(Motorway)를 나중에 그려서(덮고) 간선 도로의 연속성을 유지합니다.
    if (['motorway', 'trunk', 'motorway_link', 'trunk_link'].includes(h)) {
      score += 80; // 📈 가장 나중에 그려짐 (최상단)
    } else if (['primary', 'primary_link', 'secondary', 'secondary_link'].includes(h)) {
      score += 50; // 중간
    } else if (['tertiary', 'tertiary_link'].includes(h)) {
      score += 30;
    } else {
      score += 10; // 📉 가장 먼저 그려짐 (바닥: 주거지 도로 등)
    }

    // [Layer Priority]
    // 물리적 높이는 스타일을 압도해야 합니다. (Layer 1 = +100점)
    // 예: Layer 1인 골목길(110점) > Layer 0인 고속도로(90점) -> 다리가 고속도로 위로 지나감 OK
    const layer = parseInt(tags.layer || '0', 10);
    score += layer * 100;
  }

  // 3. ⚡ 플래그 기반 가중치
  if (flags.isOcean) score += 100;
  if (flags.isInstitutional) score += 40;

  // 4. 🚩 랜드마크 가중치
  if (tags.tourism === 'attraction' || tags.heritage) score += 60;
  if (tags.amenity === 'hospital' || tags.amenity === 'university') score += 30;

  // 5. 📝 정보 가시성 보정
  if (tags.name) score += 20;

  // 층위 표현을 위해 255 초과 허용
  return score;
}

// =============================================================================
// SECTION 3. [Cell Policy] 🎨 셀 시각화 정책 (Baker 연동)
// * OSM 데이터를 어떤 셀 타입(Color)과 채널(Channel)로 그릴지 결정합니다.
// =============================================================================

/**
 * 🎨 [Cell Configuration]
 * 결정된 셀 타입과 타겟 채널 정보
 */
export interface CellConfig {
  cellType: number;
  channelKey: TileSurfaceKey;
}

/**
 * [Main Policy] Feature의 속성을 분석하여 그릴 셀 타입과 타겟 채널을 반환합니다.
 * @returns 설정값이 없으면 null 반환 (렌더링 스킵)
 *
 * [Channeling Strategy]
 * Detection(파싱) 순서는 구체적(Road/Building) -> 포괄적(Land) 순으로 진행하지만,
 * 렌더링(Layering) 순서는 Land -> Water -> Structure -> Transport 순입니다.
 * 아래 주석 번호(1~4)는 렌더링 패스(Channel Index)를 의미합니다.
 */
export const determineCellConfig = (category: string, tags: OsmTags, extra: OsmExtra): CellConfig | null => {
  // 1. 🏔️ Pass 1: LAND Channel (Sea & Foundations)
  if (extra.isOcean || category === 'sea') {
    return { cellType: BASE_TYPES.LAND.SEA, channelKey: 'land' };
  }

  // 2. 💧 Pass 2: WATER Channel (Inland Water)
  if (category === 'water' || tags.waterway || tags.natural === 'water' || tags.wetland || ['reservoir', 'basin'].includes(tags.landuse || '')) {
    if (tags.natural === 'wetland' || tags.wetland || tags.water === 'mud') {
      return { cellType: BASE_TYPES.WATER.WETLAND, channelKey: 'water' }; // [cite: 25]
    }

    const isFlowing = !!tags.waterway || ['river', 'canal', 'stream', 'drain'].includes(tags.water || ''); // [cite: 18] default.style의 stream, canal, drain 등 흐르는 물 판별

    const waterValue = isFlowing ? BASE_TYPES.WATER.RIVER : BASE_TYPES.WATER.LAKE;
    return { cellType: waterValue, channelKey: 'water' };
  }

  // 3. 🏢 Pass 3: STRUCTURE Channel
  if (category === 'building' || tags.building) {
    const amenity = tags.amenity || '';
    const building = tags.building || '';

    let cellType: number = BASE_TYPES.STRUCTURE.BUILDING;

    // 🚉 교통 시설
    if (
      tags.railway === 'station' ||
      tags.public_transport === 'station' ||
      amenity === 'bus_station' ||
      ['train_station', 'transportation', 'station'].includes(building)
    ) {
      cellType = BASE_TYPES.STRUCTURE.BUILDING_STATION;
    }
    // 🏥 의료 시설
    else if (MEDICAL_TYPES.has(amenity) || MEDICAL_TYPES.has(building)) {
      cellType = BASE_TYPES.STRUCTURE.BUILDING_MEDICAL;
    }
    // 🏫 교육 시설
    else if (EDU_TYPES.has(amenity) || EDU_TYPES.has(building)) {
      cellType = BASE_TYPES.STRUCTURE.BUILDING_EDUCATION;
    }
    // 🏛️ 기타 공공 시설
    else if (extra.isInstitutional) {
      cellType = BASE_TYPES.STRUCTURE.BUILDING_PUBLIC;
    }

    return { cellType, channelKey: 'structure' };
  }

  // 4. 🛣️ Pass 4: TRANSPORT Channel
  if (category === 'road' || tags.highway || tags.railway || category === 'parking') {
    let cellType: number = BASE_TYPES.TRANSPORT.ROAD_DEFAULT;

    // ✈️ 공항 선형
    if (tags.aeroway === 'runway') cellType = BASE_TYPES.TRANSPORT.RUNWAY;
    else if (tags.aeroway === 'taxiway' || tags.aeroway === 'taxilane') cellType = BASE_TYPES.TRANSPORT.TAXIWAY;
    // 주차장
    else if (extra.isParking) cellType = BASE_TYPES.TRANSPORT.PARKING;
    // 철도
    else if (tags.railway) cellType = determineRailway(tags);
    // 보행자 도로
    else if (['pedestrian', 'footway', 'living_street', 'path'].includes(tags.highway || '')) cellType = BASE_TYPES.TRANSPORT.PEDESTRIAN;
    // 도로
    else cellType = determineRoad(tags.highway || '');

    return { cellType, channelKey: 'transport' };
  }

  // 5. 🏔️ Pass 1 (Cont.): LAND Channel Fallback
  // 건물이 아니고, 도로가 아닌 남은 면(Area) 데이터들은 지형(Land)으로 처리합니다.
  const landType = resolveLand(category, tags, extra);
  if (landType) {
    return { cellType: landType, channelKey: 'land' };
  }

  return null; // 렌더링 대상 아님
};

/**
 * [Internal Helper] Land Channel의 상세 셀 타입 결정
 */
function resolveLand(category: string, tags: OsmTags, extra: OsmExtra): number | null {
  // resolveLand 내부는 기존 분화된 ID를 유지하며,
  // 시스템의 LAND 섹션 상수와 1:1 대응하도록 둡니다.

  // (0) 단순 카테고리 매핑 (빠른 반환)
  const CATEGORY_MAP: Record<string, number> = {
    forest: BASE_TYPES.LAND.GREEN_DEEP,
    park: BASE_TYPES.LAND.GREEN_LIGHT,
    sand: BASE_TYPES.LAND.SAND,
  };
  if (CATEGORY_MAP[category]) return CATEGORY_MAP[category];

  const amenity = tags.amenity || '';
  const landuse = tags.landuse || '';

  // 🏫 학교 부지
  if (EDU_TYPES.has(amenity) || landuse === 'education') return BASE_TYPES.LAND.EDUCATION;
  // 🏥 병원 부지
  if (MEDICAL_TYPES.has(amenity) || landuse === 'medical') return BASE_TYPES.LAND.MEDICAL;
  // 🏛️ 공공/교육 시설 부지
  if (extra.isInstitutional) return BASE_TYPES.LAND.PUBLIC;
  // ⚽ 스포츠 시설 부지
  if (extra.isSports) return BASE_TYPES.LAND.SPORTS;

  // (2) [Forest/Deep Green]
  if ((tags.natural && ['wood', 'scrub', 'heath', 'peak', 'ridge', 'cliff', 'mountain', 'volcano'].includes(tags.natural)) || tags.landuse === 'forest') {
    return BASE_TYPES.LAND.GREEN_DEEP;
  }
  // (3) [Grass/Light Green]
  if (
    (tags.natural && ['grassland', 'fell', 'moor'].includes(tags.natural)) ||
    (tags.landuse && ['grass', 'village_green', 'meadow', 'allotments'].includes(tags.landuse)) ||
    (tags.leisure && ['park', 'garden', 'nature_reserve'].includes(tags.leisure))
  ) {
    return BASE_TYPES.LAND.GREEN_LIGHT;
  }

  // (4) [Sand/Rock]
  if ((tags.natural && ['sand', 'shingle', 'bare_rock', 'beach', 'scree'].includes(tags.natural)) || tags.landuse === 'quarry') {
    return BASE_TYPES.LAND.SAND;
  }

  // (5) [Human Activity]
  if (landuse) {
    if (['residential', 'cemetery'].includes(landuse)) return BASE_TYPES.LAND.RESIDENTIAL;
    if (['commercial', 'retail'].includes(landuse)) return BASE_TYPES.LAND.COMMERCIAL;
    if (['industrial', 'construction'].includes(landuse)) return BASE_TYPES.LAND.INDUSTRIAL;
  }

  // (6) [Sports Fallback]
  if (tags.leisure && ['pitch', 'playground', 'golf_course', 'track', 'stadium'].includes(tags.leisure)) {
    return BASE_TYPES.LAND.SPORTS;
  }
  // (6.5) ✈️ [Airport Area]
  if (tags.aeroway === 'apron') return BASE_TYPES.LAND.APRON;
  if (tags.aeroway === 'helipad') return BASE_TYPES.LAND.HELIPAD;
  if (tags.aeroway === 'aerodrome') return BASE_TYPES.LAND.BARE; // 공항 부지 전체

  // (7) [General Fallback]
  if (tags.landuse || tags.natural || tags.leisure || tags.highway || tags.building) {
    return BASE_TYPES.LAND.BARE;
  }

  return null;
}
