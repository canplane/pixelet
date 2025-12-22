import { create } from 'zustand';
import { projection } from '@/shared/lib/geo/projection';
import { VOXEL_CONFIG } from './voxel-config';
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

/**
 * 🌐 [좌표 시스템 정의 가이드]
 * * 1. center (절대 주소 / 스냅 NO)
 * - 실제 지구상의 위경도를 미터로 변환한 "진짜 주소".
 * - 부드러운 카메라 이동과 정밀한 GPS 추적을 위해 소수점 단위를 그대로 유지함.
 * * 2. origin (절대 주소 / 스냅 YES)
 * - 월드가 생성된 시점의 "기준 주소".
 * - 격자(Grid) 시스템의 시작점이므로 반드시 VOXEL_SIZE(4)의 배수여야 함.
 * * 3. originOffset (절대 주소 / 스냅 YES)
 * - 현재 Three.js Scene의 (0,0,0) 지점이 실제 지구상에서 어디에 위치하는지 나타내는 값.
 * - 카메라 리셋 시 4m 단위로 점프하며, 이 값이 스냅되어야 배경 그리드가 흔들리지 않음.
 * * 4. 로컬 좌표 (상대 주소 / 계산값)
 * - Three.js 공간에서 실제로 메쉬가 배치되는 좌표.
 * - 계산식: [로컬 좌표 = 절대 주소 - originOffset]
 */

interface MapState {
  status: MapDataStatus;
  setStatus: (status: MapDataStatus) => void;
  loadingStep: 'roads' | 'poi' | 'complete' | null;
  setLoadingStep: (loadingStep: MapState['loadingStep']) => void;

  /** 3D 공간의 중심 좌표 (단위: 미터) */
  center: MeterPos | null;
  setCenter: (pos: MeterPos) => void;
  setCenterByLocation: (location: Location) => void;

  /** 실시간 카메라 시각 상태 */
  viewState: ViewState;
  /** 카메라 시각 상태 업데이트 (일부 혹은 전체) */
  setViewState: (view: Partial<ViewState>) => void;

  /** 지도 초기화 시점의 기준 위경도 (초기 위치 기억용) */
  origin: MeterPos | null;
  setOrigin: (pos: MeterPos) => void;
  setOriginByLocation: (location: Location) => void;

  originOffset: MeterPos;
  setOriginOffset: (offset: MeterPos) => void;

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

  /** 📍 카메라 타겟 설정 (절대 주소 / 소수점 유지) */
  center: null,
  setCenter: (pos) => set({ center: pos }),
  setCenterByLocation: (location) => {
    const meterPos = get().project(location);
    set({ center: meterPos });
  },

  /** 실시간 뷰 상태 */
  viewState: {
    distance: 200, // 카메라 거리
    bearing: 0, // 정북향
    polarAngle: DEFAULT_POLAR_ANGLE,
    // pitch: 60, // 30도 틸트 (지면 기준)
    altitude: 100, // 수직 고도
  },
  setViewState: (view) =>
    set((state) => ({
      viewState: { ...state.viewState, ...view },
    })),

  /** 🏁 월드 원점 설정 (절대 주소 / 4m 스냅 강제) */
  origin: null,
  setOrigin: (pos) => {
    // 🚀 [Rule] 이제 VOXEL_CONFIG에 정의된 snapToVoxel를 직접 사용합니다.
    const snappedPos = VOXEL_CONFIG.snapToVoxel(pos);

    set({
      origin: snappedPos, // 원점도 격자에 맞춤
      originOffset: snappedPos, // 초기 렌더링 오프셋도 동일하게 정렬
    });
  },
  setOriginByLocation: (location) => {
    const meterPos = get().project(location);
    get().setOrigin(meterPos); // 내부에서 snapToVoxel가 적용됨
  },

  /** 징검다리 역할을 하는 현재 원점 오프셋 (절대 주소 / 4m 스냅 강제) */
  originOffset: { x: 0, y: 0, z: 0 },
  setOriginOffset: (offset) => {
    // 🚀 카메라 리셋 시에도 유틸리티를 사용하여 일관성을 유지합니다.
    set({ originOffset: VOXEL_CONFIG.snapToVoxel(offset) });
  },

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
      alt: pos.y,
      lat: projection.yToLat(-pos.z),
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
