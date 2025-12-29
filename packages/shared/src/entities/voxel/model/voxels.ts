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
  RESIDENTIAL: 20, // 🏠 주거지
  COMMERCIAL: 21, // 🏬 상업지
  INDUSTRIAL: 22, // 🏭 공업지 (공사현장 포함)
  INSTITUTIONAL: 23, // 🏫 공공기관 (학교, 병원, 대학 캠퍼스)
  SPORTS: 24, // ⚽ 운동장, 골프장 (인조 잔디)

  // --- [INFRASTRUCTURE] 인프라 및 교통 ---
  ROAD: 40, // 🛣️ 도로 (차도)
  PEDESTRIAN: 60, // 🚶 보행로 (광장)
  RAIL: 70, // 🚂 철도
  PARKING: 80, // 🅿️ 주차장 (아스팔트)

  // --- [STRUCTURE] 개체 ---
  BUILDING: 100, // 🏢 건물 (메쉬화 대상)
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

  // LANDUSE
  RESIDENTIAL: '#DADADA', // 🏠 주거지: 차분한 무채색 그레이
  COMMERCIAL: '#F2DADE', // 🏬 상업지: 은은한 파스텔 핑크
  INDUSTRIAL: '#ECDBE8', // 🏭 공업지: 부드러운 라일락 그레이
  INSTITUTIONAL: '#EBEBEB', // 🏫 학교/병원: 깨끗하고 단정한 오프화이트
  SPORTS: '#BCEAC3', // ⚽ 운동장: 잔디와 어울리는 차분한 세이지 그린

  // INFRASTRUCTURE
  ROAD: '#FFFFFF', // 🛣️ 도로: 깔끔하고 정돈된 순백색
  PEDESTRIAN: '#DDDDDD', // 🚶 보행로: 도로와 구분되는 연회색
  RAIL: '#999999', // 🚂 철도: 묵직한 다크 그레이
  PARKING: '#C2C2C2', // 🅿️ 주차장: 아스팔트 느낌의 미디엄 그레이

  // STRUCTURE
  BUILDING: '#D9D0C9', // 🏢 건물: 따뜻한 느낌의 웜 그레이
} as const;
