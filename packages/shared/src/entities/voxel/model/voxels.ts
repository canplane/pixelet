/**
 * @file voxels.ts
 * 복셀의 종류와 상수 값을 정의합니다.
 */

export const VOXEL_TYPES = {
  // --- [CORE] 기본 공간 및 지형 시스템 ---
  AIR: 0, // 💨 공중 (완전한 비어있음, 초기화 값)
  UNKNOWN: 1, // 🌫️ 알 수 없는 지표면 (데이터 부재 지역의 Fallback)
  BASE_LAND: 2, // 🏜️ 일반 대지 (명시적으로 정의된 땅)
  WATER: 3, // 💧 물 (하천, 바다)

  // --- [NATURAL] 자연 지형 ---
  GREEN_DEEP: 10, // 🌲 숲/산
  GREEN_LIGHT: 11, // 🌿 공원/잔디
  SAND: 12, // 🏖️ 모래/암석

  // --- [LANDUSE] 용도별 구역 ---
  LANDUSE_RESIDENTIAL: 20, // 🏠 주거지
  LANDUSE_COMMERCIAL: 21, // 🏬 상업지
  LANDUSE_INDUSTRIAL: 22, // 🏭 공업지 (공사현장 포함)
  LANDUSE_SPORTS: 23, // ⚽ 운동장, 골프장 (인조 잔디)
  LANDUSE_EDUCATION: 24, // 🏫 교육 구역 (캠퍼스 바닥): BUILDING_EDUCATION과 조화로운 연한 베이지-아이보리
  LANDUSE_MEDICAL: 25, // 🏥 의료 구역 (병원 부지): BUILDING_MEDICAL과 조화로운 아주 연한 민트/블루
  LANDUSE_PUBLIC: 26, // 🏛️ 공공 구역 (관공서): BUILDING_PUBLIC과 조화로운 연한 그레이

  // --- [INFRASTRUCTURE] 인프라 및 교통 ---
  ROAD_DEFAULT: 40, // ⚪ 일반/주거지 도로 (소로) - 기존 ROAD 역할
  ROAD_MOTORWAY: 41, // 🔴 고속도로/간선도로 (가장 중요)
  ROAD_PRIMARY: 42, // 🟠 주요 도로 (대로)
  ROAD_SECONDARY: 43, // 🟡 보조 도로 (중로)

  PEDESTRIAN: 60, // 🚶 보행로 (광장)
  RAIL_MAIN: 70, // 🚂 일반 철도 (기차, KTX 등)
  RAIL_SUBWAY: 71, // 지하철 / 도시철도
  PARKING: 80, // 🅿️ 주차장 (아스팔트)

  // --- [✈️ AEROWAY] 공항 시설 ---
  RUNWAY: 90, // 🛫 활주로 (가장 어둡고 넓음)
  TAXIWAY: 91, // 🚕 유도로 (활주로 연결)
  APRON: 92, // 🅿️ 계류장 (비행기 주차, 넓은 콘크리트)
  HELIPAD: 93, // 🚁 헬기장

  // --- [STRUCTURE] 개체 ---
  BUILDING: 100, // 🏢 일반 건물 (주거/상업 복합)
  BUILDING_STATION: 101, // 🚉 역사/터미널 (교통 허브)
  BUILDING_EDUCATION: 102, // 🏫 학교/대학 (교육 시설)
  BUILDING_MEDICAL: 103, // 🏥 병원 (의료 시설)
  BUILDING_PUBLIC: 104, // 🏛️ 관공서 건물
} as const;

export type VoxelType = (typeof VOXEL_TYPES)[keyof typeof VOXEL_TYPES];

/**
 * [voxel-colors]
 * 지형 타입별 실제 렌더링 색상을 정의합니다.
 */
export const VOXEL_COLORS = {
  // CORE
  AIR: 'transparent', // 🎯 AIR는 렌더링 시 투명하거나 무시됨
  UNKNOWN: '#E0DDD5', // 🎯 기존 EMPTY 색상을 PLAIN이 계승 // 🌑 빈 공간: 배경과 어우러지는 샌드 그레이
  BASE_LAND: '#F2EFE9', // 🏜️ 땅: 자연스러운 미색 (크림 톤)
  WATER: '#AACBD6', // 💧 물: 차분하고 맑은 파스텔 블루

  // NATURAL
  GREEN_DEEP: '#ADD19E', // 🌲 숲: 눈이 편안한 빈티지 그린
  GREEN_LIGHT: '#C8FACA', // 🌿 공원: 부드러운 파스텔 민트
  SAND: '#F1E6B9', // 🏖️ 모래: 톤 다운된 베이지

  // LANDUSE (바닥)
  LANDUSE_RESIDENTIAL: '#DADADA', // 🏠 주거지: 차분한 무채색 그레이
  LANDUSE_COMMERCIAL: '#F2DADE', // 🏬 상업지: 은은한 파스텔 핑크
  LANDUSE_INDUSTRIAL: '#ECDBE8', // 🏭 공업지: 부드러운 라일락 그레이
  LANDUSE_SPORTS: '#BCEAC3', // ⚽ 운동장: 잔디와 어울리는 차분한 세이지 그린
  LANDUSE_EDUCATION: '#FAF7E6', // 교육 구역 바닥: 건물(#F5DEB3)보다 더 연한 상아색 (부드러운 캠퍼스 느낌)
  LANDUSE_MEDICAL: '#F0F9F5', // 의료 구역 바닥: 건물(#FFFFFF) 아래 깔리는 차분하고 청결한 연민트
  LANDUSE_PUBLIC: '#F0F2F5', // 공공 구역 바닥: 차분한 쿨 그레이 (신뢰감)- 건물보다 훨씬 밝게

  // INFRASTRUCTURE
  ROAD_DEFAULT: '#FFFFFF', // ⚪ 일반 도로: 깔끔한 순백색 (기존 유지)
  ROAD_MOTORWAY: '#E5989B', // 🔴 고속도로: 눈에 띄는 소프트 레드/핑크 (OSM의 Motorway 색상 계열)
  ROAD_PRIMARY: '#FAD492', // 🟠 주요 도로: 따뜻한 살구색/오렌지 (OSM의 Primary 색상 계열)
  ROAD_SECONDARY: '#FEFAE0', // 🟡 보조 도로: 아주 연한 레몬 크림색 (OSM의 Secondary 색상 계열)

  PEDESTRIAN: '#DDDDDD', // 🚶 보행로: 도로와 구분되는 연회색
  RAIL_MAIN: '#999999', // 🚂 철도: 묵직한 다크 그레이
  RAIL_SUBWAY: '#2A9D8F', // 지하철 / 도시철도: 청록색 계열 (도시적인 지하철 느낌)
  PARKING: '#C2C2C2', // 🅿️ 주차장: 아스팔트 느낌의 미디엄 그레이

  // ✈️ AEROWAY Colors
  RUNWAY: '#333333', // 🛫 활주로: 아주 짙은 다크 그레이 (아스팔트/타이어 자국)
  TAXIWAY: '#555555', // 🚕 유도로: 활주로보다 살짝 밝은 그레이
  APRON: '#E0E0E0', // 🅿️ 계류장: 밝은 콘크리트 색상 (Base Land와 구분)
  HELIPAD: '#555555', // 🚁 헬기장: 유도로와 동일

  // STRUCTURE (건물)
  BUILDING: '#D9D0C9', // 🏢 일반: 웜 그레이 (Warm) -> 주거지 느낌
  BUILDING_STATION: '#546E7A', // 🚉 역사: 도시적인 인프라 느낌의 '블루 그레이' (강철/유리 느낌)
  BUILDING_MEDICAL: '#FFFFFF', // 🏥 병원: 가장 밝고 깨끗한 '순백색' (위생/청결)
  BUILDING_EDUCATION: '#F5DEB3', // 🏫 학교: 따뜻한 '샌드/살구색' (벽돌/운동장 느낌)
  BUILDING_PUBLIC: '#DFE4EA', // 🏛️ 관공서: 스톤 그레이 (Cool & Solid) -> 공공 기관
} as const;
