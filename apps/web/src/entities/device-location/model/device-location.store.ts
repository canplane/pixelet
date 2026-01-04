import { create } from 'zustand';
import type { Coord4326 } from '@pixelet/core/shared/model/coords';

/**
 * [device-location-store]
 * 기기의 물리적 센서(GPS, 나침반) 데이터와 수신 상태를 관리하는 전역 스토어입니다.
 */

export type DeviceLocationStatus = 'idle' | 'loading' | 'ready' | 'error';

interface DeviceLocationState {
  // --- 1. [State: Geolocation] 위경도 및 고도 데이터 ---
  /** 기기의 현재 위경도 및 고도 */
  location: Coord4326 | null;
  /** GPS 정확도 (단위: 미터) */
  accuracy: number | null;
  /** 이동 방향 (GPS 이동 궤적 기반, 0~360도) */
  gpsHeading: number | null;

  // --- 2. [State: Orientation] 나침반 및 권한 상태 ---
  /** 시선 방향 (마그네토미터 센서 기준, 0~360도) */
  compassHeading: number | null;
  /** 나침반 리스너 등록 및 작동 여부 */
  isOrientationStarted: boolean;

  // --- 3. [State: Lifecycle] 공통 상태 및 메시지 ---
  /** 전체적인 위치 서비스 수신 상태 */
  status: DeviceLocationStatus;
  /** 시스템 메시지 (에러 내용 등) */
  message: string | null;

  // --- 4. [Actions: Geolocation] ---
  /** GPS 데이터 수신 시 필터링 후 업데이트 */
  setGpsData: (data: {
    location: Coord4326;
    accuracy: number;
    heading: number | null;
  }) => void;

  // --- 5. [Actions: Orientation] ---
  /** 나침반 센서 데이터 업데이트 (데드존 필터링 적용) */
  setCompassHeading: (heading: number | null) => void;
  /** 나침반 활성화 상태 변경 */
  setOrientationStarted: (started: boolean) => void;

  // --- 6. [Actions: Common] ---
  /** 서비스 상태 변경 (idle, loading, ready) */
  setStatus: (status: DeviceLocationStatus) => void;
  /** 예외 상황 발생 시 에러 기록 */
  setError: (message: string) => void;
}

export const useDeviceLocationStore = create<DeviceLocationState>((set) => ({
  location: null,
  accuracy: null,
  gpsHeading: null,
  compassHeading: null,
  isOrientationStarted: false,
  status: 'idle',
  message: null,

  /**
   * [set-gps-data]
   * 하드웨어에서 전달된 원시 GPS 데이터를 검증하고, 유의미한 변화일 때만 상태를 갱신합니다.
   */
  setGpsData: (data) =>
    set((state) => {
      // 1-1. 신뢰도 가드: 정확도가 100m를 초과하는 부정확한 데이터는 무시합니다.
      if (data.accuracy > 100) return state;

      const prevLoc = state.location;
      const newLoc = data.location;

      // 1-2. 초기 수신 데이터는 즉시 반영
      if (!prevLoc) {
        return {
          location: newLoc,
          accuracy: data.accuracy,
          gpsHeading: data.heading,
          status: 'ready',
          message: null,
        };
      }

      /**
       * 1-3. [Deadzone-Filtering]
       * GPS 특유의 미세한 좌표 떨림(Jitter)이 복셀 엔진의 렌더링 부하로 이어지는 것을 방어합니다.
       * 기준: 위경도 변화 약 1.1m (0.00001) 또는 고도 변화 0.5m 미만은 무시합니다.
       */
      const latDiff = Math.abs(prevLoc.lat - newLoc.lat);
      const lngDiff = Math.abs(prevLoc.lng - newLoc.lng);
      const altDiff = Math.abs((prevLoc.alt ?? 0) - (newLoc.alt ?? 0));

      const isSignificantChange =
        latDiff > 0.00001 || lngDiff > 0.00001 || altDiff > 0.5;

      // 1-4. 변화가 없으면서 정확도 값도 동일하다면 참조 유지 (리렌더링 차단)
      if (!isSignificantChange && state.accuracy === data.accuracy) {
        return state;
      }

      return {
        location: isSignificantChange ? newLoc : prevLoc,
        accuracy: data.accuracy,
        gpsHeading: data.heading,
        status: 'ready',
        message: null,
      };
    }),

  /**
   * [set-compass-heading]
   * 기기 방향 데이터를 업데이트합니다.
   */
  setCompassHeading: (heading) =>
    set((state) => {
      if (heading === null) return { compassHeading: null };

      // 2-1. [Compass-Deadzone] 1.5도 미만의 미세한 회전 떨림은 무시하여 화면 안정성을 확보합니다.
      const prevHeading = state.compassHeading;
      if (prevHeading !== null && Math.abs(prevHeading - heading) < 1.5) {
        return state;
      }

      return { compassHeading: heading };
    }),

  /**
   * [set-orientation-started]
   * 싱글톤 훅의 중복 실행 방지를 위해 나침반 리스너 가동 상태를 기록합니다.
   */
  setOrientationStarted: (started) =>
    set((state) => {
      // 🛡️ 값이 이미 같다면 리렌더링을 발생시키지 않음
      if (state.isOrientationStarted === started) return state;
      return { isOrientationStarted: started };
    }),

  /**
   * [set-status]
   * 서비스 라이프사이클 상태를 안전하게 변경합니다.
   */
  setStatus: (status) =>
    set((state) => {
      // 🛡️ 상태가 동일하면 state 참조를 그대로 반환 (리액트 최적화)
      if (state.status === status) return state;
      return { status };
    }),

  /**
   * [set-error]
   * 하드웨어 접근 실패나 권한 문제 발생 시 에러 상태로 전환합니다.
   */
  setError: (message) =>
    set((state) => {
      if (state.status === 'error' && state.message === message) return state;
      return { status: 'error', message };
    }),
}));
