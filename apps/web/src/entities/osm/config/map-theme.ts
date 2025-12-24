export type ThemeKey = 'morning' | 'day' | 'evening' | 'night';

/**
 * [voxel-colors]
 * 지형 타입별 실제 렌더링 색상을 정의합니다.
 */
export const VOXEL_COLORS = {
  WATER: '#AACBD6',
  ROAD: '#FFFFFF',
  BUILDING: '#D9D0C9',
  GREEN_DEEP: '#ADD19E',
  GREEN_LIGHT: '#C8FACA',
  BASE_LAND: '#F2EFE9',
  RESIDENTIAL: '#DADADA',
  COMMERCIAL: '#F2DADE',
  INDUSTRIAL: '#ECDBE8',
  SAND: '#F1E6B9',
  RAIL: '#999999',
  PEDESTRIAN: '#DDDDDD',
  EMPTY: '#E0DDD5',
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
