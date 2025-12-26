/**
 * @file voxels.ts
 * 복셀의 종류와 상수 값을 정의합니다.
 */

export const VOXEL_TYPES = {
  EMPTY: 0,
  WATER: 1,
  ROAD: 2,
  BUILDING: 3,
  GREEN_DEEP: 4, // 숲, 산
  GREEN_LIGHT: 5, // 공원, 잔디
  BASE_LAND: 6, // 기본 땅
  RESIDENTIAL: 7, // 주거지
  COMMERCIAL: 8, // 상업지
  INDUSTRIAL: 9, // 공업/공사현장
  SAND: 10, // 모래/암석
  PEDESTRIAN: 11, // 보행로/광장
  RAIL: 12, // 철도

  // ✨ [New Types] 세분화 추가
  INSTITUTIONAL: 13, // 학교, 병원, 대학 (캠퍼스)
  PARKING: 14, // 주차장 (아스팔트)
  SPORTS: 15, // 운동장, 골프장
} as const;

export type VoxelType = (typeof VOXEL_TYPES)[keyof typeof VOXEL_TYPES];

/**
 * [voxel-colors]
 * 지형 타입별 실제 렌더링 색상을 정의합니다.
 */
export const VOXEL_COLORS = {
  // 💧 물: 차분하고 맑은 파스텔 블루
  WATER: '#AACBD6',

  // 🛣️ 도로: 깔끔하고 정돈된 순백색
  ROAD: '#FFFFFF',

  // 🏢 건물: 따뜻한 느낌의 웜 그레이
  BUILDING: '#D9D0C9',

  // 🌲 숲: 눈이 편안한 빈티지 그린
  GREEN_DEEP: '#ADD19E',

  // 🌿 공원: 부드러운 파스텔 민트
  GREEN_LIGHT: '#C8FACA',

  // 🏜️ 땅: 자연스러운 미색 (크림 톤)
  BASE_LAND: '#F2EFE9',

  // 🏠 주거지: 차분한 무채색 그레이
  RESIDENTIAL: '#DADADA',

  // 🏬 상업지: 은은한 파스텔 핑크
  COMMERCIAL: '#F2DADE',

  // 🏭 공업지: 부드러운 라일락 그레이
  INDUSTRIAL: '#ECDBE8',

  // 🏖️ 모래: 톤 다운된 베이지
  SAND: '#F1E6B9',

  // 🚂 철도: 묵직한 다크 그레이
  RAIL: '#999999',

  // 🚶 보행로: 도로와 구분되는 연회색
  PEDESTRIAN: '#DDDDDD',

  // 🌑 빈 공간: 배경과 어우러지는 샌드 그레이
  EMPTY: '#E0DDD5',

  // 🏫 학교/병원: 깨끗하고 단정한 오프화이트
  INSTITUTIONAL: '#EBEBEB',

  // 🅿️ 주차장: 아스팔트 느낌의 미디엄 그레이
  PARKING: '#C2C2C2',

  // ⚽ 운동장: 잔디와 어울리는 차분한 세이지 그린
  SPORTS: '#BCEAC3',
} as const;
