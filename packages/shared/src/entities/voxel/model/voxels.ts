/**
 * @file voxels.ts
 * 복셀의 종류와 상수 값을 정의합니다.
 * * [Refactor Note]
 * - Major(Channel) / Minor(Type) 2단계 위계 적용
 * - 각 채널은 독립적인 ID 네임스페이스(Uint8, 0~255)를 가집니다.
 */

// =============================================================================
// 1. Voxel ID Definitions (Major -> Minor)
// =============================================================================

export const VOXEL_TYPES = {
  /** * 🌐 [COMMON]
   * 모든 채널에서 공통으로 사용하는 상태 값입니다.
   */
  COMMON: {
    AIR: 0, // 💨 공중 (완전한 비어있음, 초기화 값)
    UNKNOWN: 1, // 🌫️ 알 수 없는 상태 (데이터 부재 지역의 Fallback)
  },

  /** * 💧 [Channel: WATER]
   * 수계 채널 (바다 + 내륙 수계 통합) (Pass 1)
   */
  WATER: {
    SEA: 2, // 🌊 바다 (Ocean, Coastline) - 배경이 되는 거대한 물
    INLAND_WATER: 3, // 🏞️ 내륙 수계 (River, Lake, Basin) - 지형 위의 물
  },

  /** * ⛰️ [Channel: BASE]
   * 지형/바닥 채널 (Natural + Landuse) (Pass 2)
   */
  BASE: {
    BASE_LAND: 2, // 🏜️ 일반 대지 (명시적으로 정의된 땅)

    // --- [NATURAL] 자연 지형 ---
    GREEN_DEEP: 10, // 🌲 숲/산
    GREEN_LIGHT: 11, // 🌿 공원/잔디
    SAND: 12, // 🏖️ 모래/암석

    // --- [LANDUSE] 용도별 구역 ---
    RESIDENTIAL: 20, // 🏠 주거지
    COMMERCIAL: 21, // 🏬 상업지
    INDUSTRIAL: 22, // 🏭 공업지 (공사현장 포함)
    SPORTS: 23, // ⚽ 운동장, 골프장 (인조 잔디)
    EDUCATION: 24, // 🏫 교육 구역 (캠퍼스 바닥): BUILDING_EDUCATION과 조화로운 연한 베이지-아이보리
    MEDICAL: 25, // 🏥 의료 구역 (병원 부지): BUILDING_MEDICAL과 조화로운 아주 연한 민트/블루
    PUBLIC: 26, // 🏛️ 공공 구역 (관공서): BUILDING_PUBLIC과 조화로운 연한 그레이

    // --- [AEROWAY AREAS] 공항 바닥 ---
    APRON: 30, // 🅿️ 계류장 (비행기 주차, 넓은 콘크리트)
    HELIPAD: 31, // 🚁 헬기장
  },

  /** * 🛣️ [Channel: TRANSPORT]
   * 교통 및 인프라 (Pass 3)
   */
  TRANSPORT: {
    // --- [ROADS] 도로 ---
    ROAD_DEFAULT: 2, // ⚪ 일반/주거지 도로 (소로)
    ROAD_MOTORWAY: 3, // 🔴 고속도로/간선도로 (가장 중요)
    ROAD_PRIMARY: 4, // 🟠 주요 도로 (대로)
    ROAD_SECONDARY: 5, // 🟡 보조 도로 (중로)
    PEDESTRIAN: 6, // 🚶 보행로 (광장)
    PARKING: 7, // 🅿️ 주차장 (아스팔트)

    // --- [RAILS] 철도 ---
    RAIL_MAIN: 10, // 🚂 일반 철도 (기차, KTX 등)
    RAIL_SUBWAY: 11, // 🚇 지하철 / 도시철도

    // --- [AEROWAY LINES] 공항 선형 ---
    RUNWAY: 20, // 🛫 활주로 (가장 어둡고 넓음)
    TAXIWAY: 21, // 🚕 유도로 (활주로 연결)
  },

  /** * 🏢 [Channel: STRUCTURE]
   * 구조물 및 입체 개체 (Pass 4)
   */
  STRUCTURE: {
    // --- [BUILDINGS] 건물 ---
    BUILDING: 2, // 🏢 일반 건물 (주거/상업 복합)
    BUILDING_STATION: 3, // 🚉 역사/터미널 (교통 허브)
    BUILDING_EDUCATION: 4, // 🏫 학교/대학 (교육 시설)
    BUILDING_MEDICAL: 5, // 🏥 병원 (의료 시설)
    BUILDING_PUBLIC: 6, // 🏛️ 관공서 건물
  },
} as const;

// =============================================================================
// 2. Voxel Color Definitions (Mapped by Major -> Minor ID)
// =============================================================================

/**
 * [voxel-colors]
 * 각 채널(Major)의 ID(Minor)에 대응하는 렌더링 색상입니다.
 * Exporter나 Renderer는 `VOXEL_COLORS[ChannelKey][LocalID]` 형태로 접근합니다.
 */
export const VOXEL_COLORS = {
  COMMON: {
    [VOXEL_TYPES.COMMON.AIR]: 'transparent', // 🎯 AIR는 투명
    [VOXEL_TYPES.COMMON.UNKNOWN]: '#E0DDD5', // 🌑 빈 공간: 배경과 어우러지는 샌드 그레이
  },
  WATER: {
    [VOXEL_TYPES.WATER.SEA]: '#AACBD6', // 🌊 바다: 기존의 차분한 파스텔 블루 (깊이감)
    [VOXEL_TYPES.WATER.INLAND_WATER]: '#C4E3ED', // 🏞️ 내륙 수계: 바다보다 조금 더 밝고 경쾌한 블루 (청량감)
  },
  BASE: {
    [VOXEL_TYPES.BASE.BASE_LAND]: '#F2EFE9', // 🏜️ 땅: 자연스러운 미색 (크림 톤)

    // Natural
    [VOXEL_TYPES.BASE.GREEN_DEEP]: '#ADD19E', // 🌲 숲: 눈이 편안한 빈티지 그린
    [VOXEL_TYPES.BASE.GREEN_LIGHT]: '#C8FACA', // 🌿 공원: 부드러운 파스텔 민트
    [VOXEL_TYPES.BASE.SAND]: '#F1E6B9', // 🏖️ 모래: 톤 다운된 베이지

    // Landuse
    [VOXEL_TYPES.BASE.RESIDENTIAL]: '#DADADA', // 🏠 주거지: 차분한 무채색 그레이
    [VOXEL_TYPES.BASE.COMMERCIAL]: '#F2DADE', // 🏬 상업지: 은은한 파스텔 핑크
    [VOXEL_TYPES.BASE.INDUSTRIAL]: '#ECDBE8', // 🏭 공업지: 부드러운 라일락 그레이
    [VOXEL_TYPES.BASE.SPORTS]: '#BCEAC3', // ⚽ 운동장: 잔디와 어울리는 차분한 세이지 그린
    [VOXEL_TYPES.BASE.EDUCATION]: '#FAF7E6', // 교육 구역 바닥: 건물(#F5DEB3)보다 더 연한 상아색
    [VOXEL_TYPES.BASE.MEDICAL]: '#F0F9F5', // 의료 구역 바닥: 건물(#FFFFFF) 아래 깔리는 차분하고 청결한 연민트
    [VOXEL_TYPES.BASE.PUBLIC]: '#F0F2F5', // 공공 구역 바닥: 차분한 쿨 그레이 (신뢰감)

    // Aeroway Areas
    [VOXEL_TYPES.BASE.APRON]: '#E0E0E0', // 🅿️ 계류장: 밝은 콘크리트 색상
    [VOXEL_TYPES.BASE.HELIPAD]: '#555555', // 🚁 헬기장: 유도로와 동일
  },
  TRANSPORT: {
    // Roads
    [VOXEL_TYPES.TRANSPORT.ROAD_DEFAULT]: '#FFFFFF', // ⚪ 일반 도로: 깔끔한 순백색
    [VOXEL_TYPES.TRANSPORT.ROAD_MOTORWAY]: '#E5989B', // 🔴 고속도로: 눈에 띄는 소프트 레드/핑크
    [VOXEL_TYPES.TRANSPORT.ROAD_PRIMARY]: '#FAD492', // 🟠 주요 도로: 따뜻한 살구색/오렌지
    [VOXEL_TYPES.TRANSPORT.ROAD_SECONDARY]: '#FEFAE0', // 🟡 보조 도로: 아주 연한 레몬 크림색
    [VOXEL_TYPES.TRANSPORT.PEDESTRIAN]: '#DDDDDD', // 🚶 보행로: 도로와 구분되는 연회색
    [VOXEL_TYPES.TRANSPORT.PARKING]: '#C2C2C2', // 🅿️ 주차장: 아스팔트 느낌의 미디엄 그레이

    // Rails
    [VOXEL_TYPES.TRANSPORT.RAIL_MAIN]: '#999999', // 🚂 철도: 묵직한 다크 그레이
    [VOXEL_TYPES.TRANSPORT.RAIL_SUBWAY]: '#2A9D8F', // 🚇 지하철 / 도시철도: 청록색 계열

    // Aeroway Lines
    [VOXEL_TYPES.TRANSPORT.RUNWAY]: '#333333', // 🛫 활주로: 아주 짙은 다크 그레이
    [VOXEL_TYPES.TRANSPORT.TAXIWAY]: '#555555', // 🚕 유도로: 활주로보다 살짝 밝은 그레이
  },
  STRUCTURE: {
    // Buildings
    [VOXEL_TYPES.STRUCTURE.BUILDING]: '#D9D0C9', // 🏢 일반: 웜 그레이
    [VOXEL_TYPES.STRUCTURE.BUILDING_STATION]: '#546E7A', // 🚉 역사: 블루 그레이 (Cool & Heavy)
    [VOXEL_TYPES.STRUCTURE.BUILDING_MEDICAL]: '#FFFFFF', // 🏥 병원: 순백색 (Bright)
    [VOXEL_TYPES.STRUCTURE.BUILDING_EDUCATION]: '#F5DEB3', // 🏫 학교: 샌드/살구 (Warm & Soft)
    [VOXEL_TYPES.STRUCTURE.BUILDING_PUBLIC]: '#DFE4EA', // 🏛️ 관공서: 스톤 그레이 (Cool & Solid)
  },
} as const;
