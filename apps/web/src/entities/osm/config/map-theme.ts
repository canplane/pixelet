export type ThemeKey = 'morning' | 'day' | 'evening' | 'night';

/**
 * [voxel-colors]
 * 지형 타입별 실제 렌더링 색상을 정의합니다.
 */
export const VOXEL_COLORS = {
  // 💧 물: 차분하고 맑은 파스텔 블루
  WATER: '#AACBD6',

  // 🛣️ 도로: 깔끔하고 정돈된 순백색
  ROAD: '#FFFFFF',

  // 🏢 건물: 배경처럼 은은하게 묻히는 연한 웜 그레이 (업데이트됨)
  BUILDING: '#E4DFD9',

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

  // ✨ [New] 구역(Boundary) 상세 색상 보강
  // 🏫 학교/병원: 깨끗하고 단정한 오프화이트 (건물보다 밝고 땅보다 하얀 톤)
  INSTITUTIONAL: '#EBEBEB',

  // 🅿️ 주차장: 아스팔트 느낌의 미디엄 그레이 (도로와 대비되는 차분한 회색)
  PARKING: '#C2C2C2',

  // ⚽ 운동장: 잔디와 어울리는 차분한 세이지 그린 (공원보다 톤 다운된 녹색)
  SPORTS: '#BCEAC3',
} as const;

/**
 * [map-layout-theme]
 * 월드 환경 설정 테마입니다. (grid 필드 제거 완료)
 */
export type MapLayoutTheme = {
  background: string;
  fog: string;
  sky: {
    rayleigh: number;
    mieCoefficient: number;
    sunPosition: [number, number, number];
  };
  lighting: {
    ambient: { color: string; intensity: number };
    hemisphere: { sky: string; ground: string; intensity: number };
    directional: { color: string; intensity: number };
  };
  grid: {
    cell: string;
    section: string;
  };
};

export const MAP_LAYOUT_THEMES: Record<ThemeKey, MapLayoutTheme> = {
  morning: {
    background: '#f8fafc',
    fog: '#e2e8f0',
    sky: { rayleigh: 3, mieCoefficient: 0.005, sunPosition: [100, 20, 100] },
    lighting: {
      ambient: { color: '#fff5e6', intensity: 1.0 },
      hemisphere: { sky: '#ffffff', ground: '#ffcc99', intensity: 1.0 },
      directional: { color: '#ffffff', intensity: 2.0 },
    },
    grid: { cell: '#e2e8f0', section: '#cbd5e1' },
  },
  day: {
    background: '#e0f2fe',
    fog: '#bae6fd',
    sky: { rayleigh: 1.5, mieCoefficient: 0.002, sunPosition: [50, 100, 50] },
    lighting: {
      ambient: { color: '#ffffff', intensity: 0.4 },
      hemisphere: { sky: '#ffffff', ground: '#94a3b8', intensity: 0.4 },
      directional: { color: '#ffffff', intensity: 0.6 },
    },
    grid: { cell: '#cbd5e1', section: '#94a3b8' },
  },
  evening: {
    background: '#451a03',
    fog: '#fb923c',
    sky: { rayleigh: 6, mieCoefficient: 0.05, sunPosition: [100, 10, -100] },
    lighting: {
      ambient: { color: '#ffedd5', intensity: 1.2 },
      hemisphere: { sky: '#fb923c', ground: '#451a03', intensity: 1.5 },
      directional: { color: '#fca5a5', intensity: 2.0 },
    },
    grid: { cell: '#78350f', section: '#451a03' },
  },
  night: {
    background: '#0f172a',
    fog: '#1e293b',
    sky: { rayleigh: 10, mieCoefficient: 0.1, sunPosition: [0, -100, 0] },
    lighting: {
      ambient: { color: '#1e293b', intensity: 0.5 },
      hemisphere: { sky: '#1e293b', ground: '#020617', intensity: 0.8 },
      directional: { color: '#334155', intensity: 0.5 },
    },
    grid: { cell: '#334155', section: '#1e293b' },
  },
};
