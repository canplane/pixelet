import { create } from 'zustand';

// 1. 수학 및 투영 유틸리티
import { projection } from '@pixelet/shared/shared/lib/map/projection';

// 2. 기초 타입 (Location, MeterPos 등)
import type { Location, MeterPos } from '@pixelet/shared/shared/model/coords';
import type { GeometryTile } from '@pixelet/shared/entities/osm/model/osm-tile';

// 3. OSM 관련 로직 및 도메인 타입
import type { MapPoi } from '@pixelet/shared/entities/osm/model/osm-feature';

// 4. 전역 설정 및 테마
import type { ThemeKey } from '../../../features/map-three/model/world-themes';

// 5. 현재 패키지(web) 내부 설정
import { VOXEL_CONFIG } from '@pixelet/shared/entities/voxel/voxel-config';
import { THREE_CONFIG } from '../config/three-config';

const { project } = projection;
const { DEFAULT_POLAR_ANGLE } = THREE_CONFIG.CAMERA;

export type MapDataStatus = 'idle' | 'loading' | 'success' | 'error';

/**
 * [view-state]
 * 카메라 및 지도의 실시간 시각적 상태를 정의합니다.
 */
export interface ViewState {
  distance: number; // 줌 레벨 (카메라-타겟 거리)
  bearing: number; // 수평 회전 각도 (0~360)
  polarAngle: number; // 수직 각도 (라디안)
  altitude: number; // 지면으로부터의 수직 높이
}

/**
 * [map-state]
 * 지도 엔진의 핵심 상태 및 좌표계를 관리합니다.
 *
 * 🌐 [floating-origin-coordinate-system]
 * 1. center (Absolute): 카메라가 보고 있는 실제 지구상의 절대 좌표 (미터)
 * 2. origin (Reference): Three.js Scene의 원점(0,0,0)에 대응되는 실제 지구상의 절대 좌표.
 * - 카메라가 멀리 이동할 때마다 타일 단위로 갱신(Shift)되어 부동소수점 정밀도를 유지합니다.
 * 3. Local Position: 렌더링 시에는 (Absolute - origin) 연산을 통해 0 근처의 로컬 좌표로 변환됩니다.
 */
interface MapState {
  status: MapDataStatus;
  setStatus: (status: MapDataStatus) => void;
  loadingStep: 'roads' | 'poi' | 'complete' | null;
  setLoadingStep: (loadingStep: MapState['loadingStep']) => void;

  /** 활성 범위 내의 타일 데이터 관리 */
  tiles: Map<string, GeometryTile>;
  /**
   * [set-geometry-tile] (Partial Update Supported)
   * 타일 데이터의 점진적 로딩을 지원합니다. (예: 도로 먼저 -> POI 나중에)
   */
  setGeometryTile: (key: string, tile: Partial<GeometryTile>) => void;
  removeGeometryTile: (key: string) => void;
  clearOldTiles: (neededKeys: string[]) => void;

  // [center-position] 카메라가 보고 있는 절대 위치
  center: MeterPos | null;
  setCenter: (pos: MeterPos) => void;
  setCenterByLocation: (location: Location) => void;

  viewState: ViewState;
  setViewState: (view: Partial<ViewState>) => void;

  // [floating-origin] 시스템의 동적 기준점
  origin: MeterPos;
  setOrigin: (origin: MeterPos) => void;

  cameraResetSignal: { distance: number; orientation: number };
  triggerDistanceReset: () => void;
  triggerOrientationReset: () => void;

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
  getThemeKey: () => ThemeKey;
}

/**
 * [use-map-store]
 * 지도 서비스의 글로벌 상태를 관리하는 커스텀 훅입니다.
 */
export const useMapStore = create<MapState>((set, get) => ({
  status: 'idle',
  setStatus: (status) => set({ status }),
  loadingStep: null,
  setLoadingStep: (loadingStep) => set({ loadingStep }),

  tiles: new Map(),

  /**
   * [set-geometry-tile]
   * 타일 데이터를 저장하거나 부분 업데이트를 수행합니다.
   */
  setGeometryTile: (key, partialTile) =>
    set((state) => {
      const newMap = new Map(state.tiles);
      const existingTile = newMap.get(key);

      if (existingTile) {
        // [merge-strategy] 기존 타일 병합
        newMap.set(key, {
          ...existingTile,
          ...partialTile,
        });
      } else {
        // [new-tile-validation] 필수 데이터 확인 후 생성
        if (partialTile.origin && partialTile.grids && partialTile.resolution) {
          newMap.set(key, partialTile as GeometryTile);
        } else {
          console.warn(
            `[MapStore] Incomplete tile data received for ${key}`,
            partialTile,
          );
        }
      }
      return { tiles: newMap };
    }),

  removeGeometryTile: (key) =>
    set((state) => {
      const newMap = new Map(state.tiles);
      newMap.delete(key);
      return { tiles: newMap };
    }),

  /**
   * [clear-old-tiles]
   * 현재 가시 범위 밖에 있는 타일들을 메모리에서 해제합니다.
   */
  clearOldTiles: (neededKeys) =>
    set((state) => {
      const newMap = new Map(state.tiles);
      let changed = false;
      newMap.forEach((_, key) => {
        if (!neededKeys.includes(key)) {
          newMap.delete(key);
          changed = true;
        }
      });
      return changed ? { tiles: newMap } : state;
    }),

  center: null,
  setCenter: (pos) => set({ center: pos }),
  setCenterByLocation: (location) => {
    const meterPos = project(location);
    set({ center: meterPos });
  },

  viewState: {
    distance: 200,
    bearing: 0,
    polarAngle: DEFAULT_POLAR_ANGLE,
    altitude: 100,
  },
  setViewState: (view) =>
    set((state) => ({
      viewState: { ...state.viewState, ...view },
    })),

  // [origin-initial-state] (0,0,0)으로 시작
  origin: { x: 0, y: 0, z: 0 },

  /**
   * [set-origin]
   * 오리진 설정 시 항상 격자(voxel) 단위로 스냅하여 렌더링 정밀도를 보장합니다.
   */
  setOrigin: (newOrigin) => {
    set({ origin: VOXEL_CONFIG.snapToVoxel(newOrigin) });
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

  portalElement: null,
  setPortalElement: (el) => set({ portalElement: el }),

  isFollowMode: false,
  setIsFollowMode: (isFollowMode) => set({ isFollowMode }),

  isInteracting: false,
  setIsInteracting: (isInteracting) => set({ isInteracting }),

  currentHour: new Date().getHours(),
  setCurrentHour: (hour: number) => set({ currentHour: hour }),

  /**
   * [get-theme-key]
   * 현재 시간에 따른 시각적 테마 키를 반환합니다.
   */
  getThemeKey: () => {
    const hour = get().currentHour;
    if (hour >= 5 && hour < 11) return 'morning';
    if (hour >= 11 && hour < 17) return 'day';
    if (hour >= 17 && hour < 21) return 'evening';
    return 'night';
  },
}));
