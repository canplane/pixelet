import { create } from 'zustand';

// 1. 수학 및 투영 유틸리티
import { projection } from '@pixelet/shared/shared/lib/map/projection';

// 2. 기초 타입 (Coord 등)
import type { Coord } from '@pixelet/shared/shared/model/coords';
import type { GeometryTile } from '@pixelet/shared/entities/osm/model/tiles';

// 3. OSM 관련 로직 및 도메인 타입
import type { MapPoi } from '@pixelet/shared/entities/osm/model/osm-feature';

// 4. 전역 설정 및 테마
import type { ThemeKey } from '../../../features/map-three/model/world-themes';

// 5. 현재 패키지(web) 내부 설정
import { VOXEL_CONFIG } from '@pixelet/shared/entities/voxel/voxel-config';
import { THREE_CONFIG } from '../config/three-config';
import {
  getTileKey,
  type TileCoord,
} from '@pixelet/shared/entities/osm/lib/utils/tile-utils';

const { DEFAULT_POLAR_ANGLE } = THREE_CONFIG.CAMERA;

/**
 * [status-lifecycle]
 * 지도의 전반적인 데이터 로딩 상태입니다.
 * - idle: 초기 대기
 * - loading: 데이터 페칭 중
 * - success: 렌더링 준비 완료
 * - error: 로딩 실패
 */
export type MapDataStatus = 'idle' | 'loading' | 'success' | 'error';

/**
 * [view-state]
 * 카메라 및 지도의 실시간 시각적 상태를 정의합니다.
 */
export interface ViewState {
  distance: number; // 줌 레벨 (카메라-타겟 거리)
  bearing: number; // 수평 회전 각도 (0~360, 북쪽 기준)
  polarAngle: number; // 수직 각도 (0:수직, PI/2:수평)
  altitude: number; // 지면으로부터의 카메라 수직 높이
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
  // --- [Lifecycle & Status] ---
  status: MapDataStatus;
  setStatus: (status: MapDataStatus) => void;
  /** [progressive-loading] 단계별 로딩 상태 (도로 -> POI -> 완료) */
  loadingStep: 'roads' | 'poi' | 'complete' | null;
  setLoadingStep: (loadingStep: MapState['loadingStep']) => void;

  // --- [Tile Data Management] ---
  /** 현재 메모리에 로드된 타일 데이터 저장소 (Key: "tx:ty") */
  tiles: Map<string, GeometryTile>;
  /**
   * [set-geometry-tile] (Partial Update Supported)
   * 타일 데이터를 새로 추가하거나, 이미 존재하는 타일에 데이터를 병합(Merge)합니다.
   * 예: Worker로부터 지형 데이터 수신 후, 나중에 POI 데이터 수신 시 병합
   */
  setGeometryTile: (coord: TileCoord, tile: Partial<GeometryTile>) => void;
  /** 특정 타일을 메모리에서 즉시 해제합니다. */
  removeGeometryTile: (key: string) => void;
  /** 가시 범위 밖의 불필요한 타일들을 일괄 정리(GC)합니다. */
  clearOldTiles: (neededKeys: string[]) => void;

  // --- [Navigation & Coordinate] ---
  /** [center-position] 카메라가 현재 바라보고 있는 절대적 중심 좌표 (Target) */
  center: Coord | null;
  setCenter: (pos: Coord) => void;

  /** 카메라의 줌, 회전, 고도 등 시각적 파라미터 */
  viewState: ViewState;
  setViewState: (view: Partial<ViewState>) => void;

  /**
   * [floating-origin]
   * 렌더링 정밀도 유지를 위해 동적으로 변하는 World 원점 (0,0,0)의 실제 절대 좌표.
   * 카메라 이동에 따라 타일 크기 단위로 스냅되어 이동합니다.
   */
  origin: Coord;
  setOrigin: (origin: Coord) => void;

  // --- [Control Signals] ---
  /** 카메라 리셋 요청을 트리거하기 위한 시그널 (Timestamp 기반) */
  cameraResetSignal: { distance: number; orientation: number };
  triggerDistanceReset: () => void;
  triggerOrientationReset: () => void;

  // --- [Interaction] ---
  /** 마우스가 호버 중인 3D 월드 상의 좌표 (Raycasting 결과) */
  hoveredPos: Coord | null;
  setHoveredPos: (pos: Coord | null) => void;

  /** 사용자가 클릭하여 선택한 POI 객체 */
  selectedPoi: MapPoi | null;
  setSelectedPoi: (poi: MapPoi | null) => void;

  /**
   * [html-portal-target]
   * Three.js 캔버스 위에 HTML 오버레이를 렌더링하기 위한 DOM 노드 참조.
   * <Html portal={...}> 컴포넌트가 마운트될 위치입니다.
   */
  htmlPortalElement: HTMLDivElement | null;
  setHtmlPortalElement: (el: HTMLDivElement | null) => void;

  /** 내비게이션 추적 모드 활성화 여부 (사용자 위치 자동 따라가기) */
  isFollowMode: boolean;
  setIsFollowMode: (isFollowMode: boolean) => void;

  /** 사용자가 지도를 조작(드래그, 줌 등) 중인지 여부 */
  isInteracting: boolean;
  setIsInteracting: (isInteracting: boolean) => void;

  // --- [Environment & Theme] ---
  /** 현재 가상 월드의 시간 (0~23) */
  currentHour: number;
  setCurrentHour: (hour: number) => void;
  /** 현재 시간에 따른 테마 키(morning, day, evening, night) 계산 함수 */
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
  setGeometryTile: (coord, partialTile) =>
    set((state) => {
      const key = getTileKey(coord);
      const newMap = new Map(state.tiles);
      const existingTile = newMap.get(key);

      if (existingTile) {
        // [merge-strategy] 기존 타일 병합 (Shallow Merge)
        newMap.set(key, {
          ...existingTile,
          ...partialTile,
        });
      } else {
        // [new-tile-validation] 신규 생성 시 필수 데이터 무결성 검사
        // grids가 반드시 존재해야 유효한 타일로 간주합니다.
        if (partialTile.grids) {
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
   * 현재 가시 범위(neededKeys)에 포함되지 않는 타일들을 제거하여 메모리를 확보합니다.
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
   * 오리진 설정 시 항상 격자(voxel) 단위로 스냅하여 렌더링 정밀도와 좌표 일관성을 보장합니다.
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

  htmlPortalElement: null,
  setHtmlPortalElement: (el) => set({ htmlPortalElement: el }),

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
