import { create } from 'zustand';
import { projection } from '@/shared/lib/geo/projection';
import type { Location, MeterPos } from '@/shared/types/map';
import type { MapPoi } from '../lib/osm-parser';
import { THREE_CONFIG } from './three-config';
import type { ThemeKey } from './map-theme';

const { DEFAULT_POLAR_ANGLE } = THREE_CONFIG.CAMERA;

export type MapDataStatus = 'idle' | 'loading' | 'success' | 'error';

/** 카메라 및 지도의 실시간 시각적 상태 */
export interface ViewState {
  /** 카메라와 중심점(Target) 사이의 직선 거리 (줌 레벨) */
  distance: number;
  /** 수평 회전 각도 (나침반용, 도 단위: 0~360) */
  bearing: number;
  /** * 카메라의 수직 각도 (라디안 단위)
   * 0: 수직 하강 (Top-down)
   * Math.PI / 2 (약 1.57): 지면 평행
   */
  polarAngle: number;
  // /** 수직 기울기 각도 (도 단위, 0: 수직 하강, 90: 지면 평행) */
  // pitch: number;
  /** 지면으로부터의 수직 높이 (UI 표시용) */
  altitude: number;
}

interface MapState {
  status: MapDataStatus;
  setStatus: (status: MapDataStatus) => void;
  loadingStep: 'roads' | 'poi' | 'complete' | null;
  setLoadingStep: (loadingStep: MapState['loadingStep']) => void;

  /** 3D 공간의 중심 좌표 (단위: 미터) */
  center: MeterPos | null;
  /** 지도 초기화 시점의 기준 위경도 (초기 위치 기억용) */
  origin: MeterPos | null;
  /** 실시간 카메라 상태 데이터 */
  viewState: ViewState;

  // --- 액션 ---
  setCenter: (pos: MeterPos) => void;
  setCenterByLocation: (location: Location) => void;
  setOrigin: (pos: MeterPos) => void;
  setOriginByLocation: (location: Location) => void;

  /** 카메라 시각 상태 업데이트 (일부 혹은 전체) */
  setViewState: (view: Partial<ViewState>) => void;

  // --- 좌표 변환 유틸리티 ---
  project: (location: Location) => MeterPos;
  unproject: (pos: MeterPos) => Location;

  cameraResetSignal: {
    distance: number;
    orientation: number;
  };
  triggerDistanceReset: () => void;
  triggerOrientationReset: () => void;

  // 현재 마우스가 호버 중인 복셀 좌표 상태
  hoveredPos: MeterPos | null;
  setHoveredPos: (pos: MeterPos | null) => void;

  selectedPoi: MapPoi | null;
  setSelectedPoi: (poi: MapPoi | null) => void;

  portalElement: HTMLDivElement | null;
  setPortalElement: (el: HTMLDivElement | null) => void;

  isFollowMode: boolean;
  setIsFollowMode: (isFollowMode: boolean) => void;

  isInteracting: boolean;
  setIsInteracting: (isInteracting: boolean) => void;

  originOffset: MeterPos;
  setOriginOffset: (offset: MeterPos) => void;

  currentHour: number;
  setCurrentHour: (hour: number) => void;

  // 현재 시간에 따른 테마 키 계산
  getThemeKey: () => ThemeKey;
}

export const useMapStore = create<MapState>((set, get) => ({
  status: 'idle',
  setStatus: (status) => set({ status }),
  loadingStep: null,
  setLoadingStep: (loadingStep) => set({ loadingStep }),

  /** 초기 상태 */
  center: null,
  origin: null,

  /** 초기 뷰 상태 (기본값) */
  viewState: {
    distance: 200, // 카메라 거리
    bearing: 0, // 정북향
    polarAngle: DEFAULT_POLAR_ANGLE,
    // pitch: 60, // 30도 틸트 (지면 기준)
    altitude: 100, // 수직 고도
  },

  setCenter: (pos) => set({ center: pos }),
  /** 위경도(Location)를 즉시 미터 좌표로 변환하여 센터 설정 */
  setCenterByLocation: (location) => {
    const meterPos = get().project(location);
    set({ center: meterPos });
  },

  setOrigin: (pos) => set({ origin: pos }),
  setOriginByLocation: (location) => {
    const meterPos = get().project(location);
    set({ origin: meterPos });
  },

  /** 실시간 뷰 상태 업데이트: 기존 상태를 유지하며 부분 업데이트 가능 */
  setViewState: (view) =>
    set((state) => ({
      viewState: { ...state.viewState, ...view },
    })),

  /** 좌표 변환: 위경도 → 절대 미터 좌표 (Three.js Z-up/Y-up 고려) */
  project: (location: Location) => {
    const { lng, lat, alt = 0 } = projection.clamp(location);

    return {
      x: projection.lngToX(lng), // 동서 (East-West)
      y: alt, // 상하 (Up-Down)
      z: -projection.latToY(lat), // 남북 (North-South)
    };
  },

  /** 좌표 변환: 절대 미터 좌표 → 위경도 */
  unproject: (pos: MeterPos) => {
    return {
      lng: projection.xToLng(pos.x),
      lat: projection.yToLat(-pos.z),
      alt: pos.y,
    };
  },

  cameraResetSignal: { distance: 0, orientation: 0 },

  triggerDistanceReset: () =>
    set((state) => ({
      cameraResetSignal: { ...state.cameraResetSignal, distance: Date.now() },
    })),
  triggerOrientationReset: () =>
    set((state) => ({
      cameraResetSignal: {
        ...state.cameraResetSignal,
        orientation: Date.now(),
      },
    })),

  hoveredPos: null,
  setHoveredPos: (pos) => set({ hoveredPos: pos }),

  selectedPoi: null,
  setSelectedPoi: (poi) => set({ selectedPoi: poi }),

  portalElement: null as HTMLDivElement | null,
  setPortalElement: (el: HTMLDivElement | null) => set({ portalElement: el }),

  isFollowMode: false,
  setIsFollowMode: (isFollowMode) => set({ isFollowMode }),

  isInteracting: false,
  setIsInteracting: (isInteracting) => set({ isInteracting }),

  // 초기값은 (0,0,0)
  originOffset: { x: 0, y: 0, z: 0 },
  setOriginOffset: (offset) => set({ originOffset: offset }),

  currentHour: new Date().getHours(),
  setCurrentHour: (hour: number) => set({ currentHour: hour }),

  // 현재 시간에 따른 테마 키 계산
  getThemeKey: () => {
    const hour = new Date().getHours();
    if (hour >= 5 && hour < 11) return 'morning';
    if (hour >= 11 && hour < 17) return 'day';
    if (hour >= 17 && hour < 21) return 'evening';
    return 'night';
  },
}));
