import { create } from 'zustand';
import { projection } from '@/shared/lib/geo/projection';
import { VOXEL_CONFIG } from './voxel-config';
import { OSM_CONFIG } from '@/entities/osm/config/osm-config';
import type { Location, MeterPos } from '@/shared/types/map';
import type { MapInfra, MapPoi } from '@/entities/osm/lib/osm-parser';
import { THREE_CONFIG } from './three-config';
import type { ThemeKey } from '../../../entities/osm/config/map-theme';

const { DEFAULT_POLAR_ANGLE } = THREE_CONFIG.CAMERA;

export type MapDataStatus = 'idle' | 'loading' | 'success' | 'error';

/**
 * [world-tile]
 * 타일 격자 시스템의 원자적 데이터 단위입니다.
 * 모든 공간 데이터는 서버 및 글로벌 기준인 '절대 미터 좌표계'를 유지합니다.
 */
export interface WorldTile {
  key: string; // "tx:tz" 형식의 고유 키
  origin: MeterPos; // 타일의 시작점 (절대 미터 좌표)
  grid: Uint8Array; // 🎯 명칭 통일: 타일 지형 격자 데이터 (Voxel Type Map)
  pois: MapPoi[]; // 타일에 포함된 절대 좌표 기반 POI
  infras: MapInfra[]; // 타일에 포함된 절대 좌표 기반 인프라
  resolution: number; // 격자 해상도
}

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
 * 전체 지도 엔진의 상태 관리 및 좌표 변환 로직을 담당합니다.
 * * 🌐 좌표 시스템 정의:
 * 1. center: 실제 지구상의 위경도를 미터로 변환한 절대 주소.
 * 2. origin: 격자 시스템의 기준점 (VOXEL_SIZE 배수 스냅).
 * 3. origin-offset: Three.js Scene (0,0,0)의 실제 지구상 위치.
 * 4. local-pos: Scene 내 배치 좌표 (절대 주소 - origin-offset).
 */
interface MapState {
  status: MapDataStatus;
  setStatus: (status: MapDataStatus) => void;
  loadingStep: 'roads' | 'poi' | 'complete' | null;
  setLoadingStep: (loadingStep: MapState['loadingStep']) => void;

  /** 활성 범위 내의 타일 데이터 관리 (Grid 중심) */
  worldTiles: Map<string, WorldTile>;
  setWorldTile: (key: string, tile: WorldTile) => void;
  removeWorldTile: (key: string) => void;
  clearOldTiles: (neededKeys: string[]) => void;

  center: MeterPos | null;
  setCenter: (pos: MeterPos) => void;
  setCenterByLocation: (location: Location) => void;

  viewState: ViewState;
  setViewState: (view: Partial<ViewState>) => void;

  origin: MeterPos | null;
  setOrigin: (pos: MeterPos) => void;
  setOriginByLocation: (location: Location) => void;

  originOffset: MeterPos;
  setOriginOffset: (offset: MeterPos) => void;

  // --- [좌표 변환 유틸리티] ---
  project: (location: Location) => MeterPos;
  unproject: (pos: MeterPos) => Location;

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

  worldTiles: new Map(),

  setWorldTile: (key, tile) =>
    set((state) => {
      const newMap = new Map(state.worldTiles);
      newMap.set(key, tile);
      return { worldTiles: newMap };
    }),

  removeWorldTile: (key) =>
    set((state) => {
      const newMap = new Map(state.worldTiles);
      newMap.delete(key);
      return { worldTiles: newMap };
    }),

  clearOldTiles: (neededKeys) =>
    set((state) => {
      const newMap = new Map(state.worldTiles);
      let changed = false;
      newMap.forEach((_, key) => {
        if (!neededKeys.includes(key)) {
          newMap.delete(key);
          changed = true;
        }
      });
      return changed ? { worldTiles: newMap } : state;
    }),

  center: null,
  setCenter: (pos) => set({ center: pos }),
  setCenterByLocation: (location) => {
    const meterPos = get().project(location);
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

  origin: null,
  setOrigin: (pos) => {
    const snappedPos = VOXEL_CONFIG.snapToVoxel(pos);
    set({
      origin: snappedPos,
      originOffset: snappedPos,
    });
  },
  setOriginByLocation: (location) => {
    const meterPos = get().project(location);
    get().setOrigin(meterPos);
  },

  originOffset: { x: 0, y: 0, z: 0 },
  setOriginOffset: (offset) => {
    set({ originOffset: VOXEL_CONFIG.snapToVoxel(offset) });
  },

  /**
   * [project]
   * 위경도(Location)를 절대 미터 좌표(MeterPos)로 투영합니다.
   */
  project: (location: Location) => {
    const { lng, lat, alt = 0 } = projection.clamp(location);
    return {
      x: projection.lngToX(lng),
      y: alt,
      z: -projection.latToY(lat),
    };
  },

  /**
   * [unproject]
   * 절대 미터 좌표를 위경도 좌표로 역투영합니다.
   */
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
