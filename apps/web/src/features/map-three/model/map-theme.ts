export type ThemeKey = 'morning' | 'day' | 'evening' | 'night';

export type MapTheme = {
  background: string; // Canvas 배경색 (div)
  fog: string; // 안개 색상
  sky: {
    rayleigh: number; // 하늘 푸른 정도
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

export const MAP_THEMES: Record<ThemeKey, MapTheme> = {
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
    background: '#f1f5f9', // 🚀 너무 하얗지 않은 연회색
    fog: '#cbd5e1',
    sky: { rayleigh: 2, mieCoefficient: 0.002, sunPosition: [0, 1000, 0] },
    lighting: {
      ambient: { color: '#ffffff', intensity: 1.2 },
      hemisphere: { sky: '#ffffff', ground: '#94a3b8', intensity: 1.2 },
      directional: { color: '#ffffff', intensity: 2.5 },
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
