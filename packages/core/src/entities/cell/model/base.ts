/**
 * @file base.ts
 * [Layer: BASE] Tile 단위로 로딩되는 2.5D 캔버스 데이터
 */

import { COMMON_TYPES, COMMON_COLORS } from './common';
import { CELL_MASKS } from './utils';

// -----------------------------------------------------------------------------
// ID Definitions
// -----------------------------------------------------------------------------

export const BASE_TYPES = {
  /** ⛰️ [Channel: LAND] 배경 및 지형 (MSB Partitioned) */
  LAND: {
    ...COMMON_TYPES, // AIR(0), UNKNOWN(255) 상속

    // --- [BASIC] ---

    /** 🏜️ 일반 대지 (기본값) */
    BARE: 1,
    /** 🌊 바다 (129) */
    SEA: 1 | CELL_MASKS.SEA_FLAG,

    // --- [NATURAL] ---

    /** 🌲 숲/산 */
    GREEN_DEEP: 10,
    /** 🌿 공원/잔디 */
    GREEN_LIGHT: 11,
    /** 🏖️ 모래/암석 */
    SAND: 12,

    // --- [LANDUSE] ---

    /** 🏠 주거지 */
    RESIDENTIAL: 20,
    /** 🏬 상업지 */
    COMMERCIAL: 21,
    /** 🏭 공업지 */
    INDUSTRIAL: 22,
    /** ⚽ 운동장 */
    SPORTS: 23,
    /** 🏫 교육 구역 */
    EDUCATION: 24,
    /** 🏥 의료 구역 */
    MEDICAL: 25,
    /** 🏛️ 공공 구역 */
    PUBLIC: 26,

    // --- [AEROWAY AREAS] ---

    /** 🅿️ 계류장 */
    APRON: 30,
    /** 🚁 헬기장 */
    HELIPAD: 31,
  },

  /** 💧 [Channel: WATER] 내륙 수계 */
  WATER: {
    ...COMMON_TYPES,

    /** 🌊 흐르는 물 */
    RIVER: 1,
    /** 🏞️ 고인 물 */
    LAKE: 2,
    /** 🌾 습지 */
    WETLAND: 3,
  },

  /** 🏢 [Channel: STRUCTURE] 구조물 */
  STRUCTURE: {
    ...COMMON_TYPES,

    // --- [BUILDINGS] ---

    /** 🏢 일반 건물 */
    BUILDING: 1,
    /** 🚉 역사/터미널 */
    BUILDING_STATION: 2,
    /** 🏫 학교 건물 */
    BUILDING_EDUCATION: 3,
    /** 🏥 병원 건물 */
    BUILDING_MEDICAL: 4,
    /** 🏛️ 관공서 건물 */
    BUILDING_PUBLIC: 5,
  },

  /** 🛣️ [Channel: TRANSPORT] 교통 */
  TRANSPORT: {
    ...COMMON_TYPES,

    // --- [ROADS] ---

    /** ⚪ 일반 도로 */
    ROAD_DEFAULT: 1,
    /** 🔴 고속도로 */
    ROAD_MOTORWAY: 2,
    /** 🟠 주요 도로 */
    ROAD_PRIMARY: 3,
    /** 🟡 보조 도로 */
    ROAD_SECONDARY: 4,
    /** 🚶 보행로 */
    PEDESTRIAN: 5,
    /** 🅿️ 주차장 */
    PARKING: 6,

    // --- [RAILS] ---

    /** 🚂 일반 철도 */
    RAIL_MAIN: 10,
    /** 🚇 지하철 */
    RAIL_SUBWAY: 11,

    // --- [AEROWAY LINES] ---

    /** 🛫 활주로 */
    RUNWAY: 20,
    /** 🚕 유도로 */
    TAXIWAY: 21,
  },
} as const;

// -----------------------------------------------------------------------------
// Color Definitions
// -----------------------------------------------------------------------------

export const BASE_COLORS = {
  /** ⛰️ [Channel: LAND] 배경 및 지형 (MSB Partitioned) */
  LAND: {
    ...COMMON_COLORS,

    // --- [BASIC] ---

    [BASE_TYPES.LAND.BARE]: '#F2EFE9', // 🏜️ 땅: 자연스러운 미색 (크림 톤)
    [BASE_TYPES.LAND.SEA]: '#AACBD6', // 🌊 바다: 기존의 차분한 파스텔 블루 (깊이감)

    // --- [NATURAL] ---

    [BASE_TYPES.LAND.GREEN_DEEP]: '#ADD19E', // 🌲 숲: 눈이 편안한 빈티지 그린
    [BASE_TYPES.LAND.GREEN_LIGHT]: '#C8FACA', // 🌿 공원: 부드러운 파스텔 민트
    [BASE_TYPES.LAND.SAND]: '#F1E6B9', // 🏖️ 모래: 톤 다운된 베이지

    // --- [LANDUSE] ---

    [BASE_TYPES.LAND.RESIDENTIAL]: '#DADADA', // 🏠 주거지: 차분한 무채색 그레이
    [BASE_TYPES.LAND.COMMERCIAL]: '#F2DADE', // 🏬 상업지: 은은한 파스텔 핑크
    [BASE_TYPES.LAND.INDUSTRIAL]: '#ECDBE8', // 🏭 공업지: 부드러운 라일락 그레이
    [BASE_TYPES.LAND.SPORTS]: '#BCEAC3', // ⚽ 운동장: 잔디와 어울리는 차분한 세이지 그린
    [BASE_TYPES.LAND.EDUCATION]: '#FAF7E6', // 교육 구역 바닥: 건물(#F5DEB3)보다 더 연한 상아색
    [BASE_TYPES.LAND.MEDICAL]: '#F0F9F5', // 의료 구역 바닥: 건물(#FFFFFF) 아래 깔리는 차분하고 청결한 연민트
    [BASE_TYPES.LAND.PUBLIC]: '#F0F2F5', // 공공 구역 바닥: 차분한 쿨 그레이 (신뢰감)

    // --- [AEROWAY AREAS] ---

    [BASE_TYPES.LAND.APRON]: '#E0E0E0', // 🅿️ 계류장: 밝은 콘크리트 색상
    [BASE_TYPES.LAND.HELIPAD]: '#555555', // 🚁 헬기장: 유도로와 동일
  },

  /** 💧 [Channel: WATER] 내륙 수계 */
  WATER: {
    ...COMMON_COLORS,

    [BASE_TYPES.WATER.RIVER]: '#B9DEEA', // 강: 흐름이 느껴지는 밝은 블루
    [BASE_TYPES.WATER.LAKE]: '#A8D3E2', // 호수: 조금 더 깊이감이 느껴지는 블루
    [BASE_TYPES.WATER.WETLAND]: '#D1E3E2', // 습지: 녹색기가 섞인 탁한 블루
  },

  /** 🏢 [Channel: STRUCTURE] 구조물 */
  STRUCTURE: {
    ...COMMON_COLORS,

    // --- [BUILDINGS] ---

    [BASE_TYPES.STRUCTURE.BUILDING]: '#D9D0C9', // 🏢 일반: 웜 그레이
    [BASE_TYPES.STRUCTURE.BUILDING_STATION]: '#546E7A', // 🚉 역사: 블루 그레이 (Cool & Heavy)
    [BASE_TYPES.STRUCTURE.BUILDING_MEDICAL]: '#FFFFFF', // 🏥 병원: 순백색 (Bright)
    [BASE_TYPES.STRUCTURE.BUILDING_EDUCATION]: '#F5DEB3', // 🏫 학교: 샌드/살구 (Warm & Soft)
    [BASE_TYPES.STRUCTURE.BUILDING_PUBLIC]: '#DFE4EA', // 🏛️ 관공서: 스톤 그레이 (Cool & Solid)
  },

  /** 🛣️ [Channel: TRANSPORT] 교통 */
  TRANSPORT: {
    ...COMMON_COLORS,

    // --- [ROADS] ---

    [BASE_TYPES.TRANSPORT.ROAD_DEFAULT]: '#FFFFFF', // ⚪ 일반 도로: 깔끔한 순백색
    [BASE_TYPES.TRANSPORT.ROAD_MOTORWAY]: '#E5989B', // 🔴 고속도로: 눈에 띄는 소프트 레드/핑크
    [BASE_TYPES.TRANSPORT.ROAD_PRIMARY]: '#FAD492', // 🟠 주요 도로: 따뜻한 살구색/오렌지
    [BASE_TYPES.TRANSPORT.ROAD_SECONDARY]: '#FEFAE0', // 🟡 보조 도로: 아주 연한 레몬 크림색
    [BASE_TYPES.TRANSPORT.PEDESTRIAN]: '#DDDDDD', // 🚶 보행로: 도로와 구분되는 연회색
    [BASE_TYPES.TRANSPORT.PARKING]: '#C2C2C2', // 🅿️ 주차장: 아스팔트 느낌의 미디엄 그레이

    // --- [RAILS] ---

    [BASE_TYPES.TRANSPORT.RAIL_MAIN]: '#999999', // 🚂 철도: 묵직한 다크 그레이
    [BASE_TYPES.TRANSPORT.RAIL_SUBWAY]: '#2A9D8F', // 🚇 지하철 / 도시철도: 청록색 계열

    // --- [AEROWAY LINES] ---

    [BASE_TYPES.TRANSPORT.RUNWAY]: '#333333', // 🛫 활주로: 아주 짙은 다크 그레이
    [BASE_TYPES.TRANSPORT.TAXIWAY]: '#555555', // 🚕 유도로: 활주로보다 살짝 밝은 그레이
  },
} as const;
