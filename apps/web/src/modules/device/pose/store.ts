// @pxl/web/modules/device/pose/store.ts
// 기기의 물리적 센서(GPS, 나침반) 데이터와 수신 상태를 관리하는 전역 스토어

import { create } from "zustand";

import type { f64 } from "@xpute/spec/lib/kit/word.ts";

import type { G4326 } from "@pxl/spec/space/adapter/4326.ts";

export const enum DevicePoseStatus {
  IDLE = "idle",
  LOADING = "loading",
  READY = "ready",
  ERROR = "error",
}

export type DeviceLocation = {
  /** 기기의 현재 위경도 및 고도 */
  coords: G4326;
  /** GPS 정확도 (단위: 미터) */
  accuracy: f64;
  /** 이동 방향 (GPS 이동 궤적 기반, 0~360도) */
  course: f64 | null;

  timestamp: f64;
};

export type DeviceOrientation = {
  /** 0~360: 나침반 방향 (Z축 회전) */
  heading: f64 | null;
  /** -180~180: 앞뒤 기울기 (X축 회전) */
  beta: f64 | null;
  /** -90~90: 좌우 기울기 (Y축 회전) */
  gamma: f64 | null;

  timestamp: f64;
};

function is_delta_over(a: f64 | null, b: f64 | null, eps: f64): boolean {
  if (a === null || b === null) return a !== b;
  return Math.abs(a - b) > eps;
}

type DeviceLocationState = {
  // ---- Geolocation: 위경도 및 고도 데이터 ----

  /** 위치 추적 활성화 여부 */
  is_tracking_location: boolean;
  /** GPS 데이터 수신 시 필터링 후 업데이트 */
  set_is_tracking_location: (enabled: boolean) => void;

  location: DeviceLocation | null;
  set_location: (coords: G4326, accuracy: f64, course: f64 | null) => void;

  // ---- Lifecycle State: 공통 상태 및 메시지 ----
  /** 전체적인 위치 서비스 수신 상태 */
  status: DevicePoseStatus;
  /** 시스템 메시지 (에러 내용 등) */
  error_msg: string | null;

  // ---- Common Actions ----
  /** 서비스 상태 변경 (idle, loading, ready) */
  set_status: (status: DevicePoseStatus) => void;
  /** 예외 상황 발생 시 에러 기록 */
  set_error: (error_msg: string) => void;
};

type DeviceOrientationState = {
  // ---- Orientation: 나침반 및 권한 상태 ----

  /** 나침반 리스너 등록 및 작동 여부 */
  is_tracking_orientation: boolean;
  /** Orientation tracking state change */
  set_is_tracking_orientation: (enabled: boolean) => void;

  orientation: DeviceOrientation | null;
  /** 나침반 센서 데이터 업데이트 (데드존 필터링 적용) */
  set_orientation: (heading: f64 | null, beta: f64 | null, gamma: f64 | null) => void;

  // ---- Lifecycle State: 공통 상태 및 메시지 ----
  /** 전체적인 위치 서비스 수신 상태 */
  status: DevicePoseStatus;
  /** 시스템 메시지 (에러 내용 등) */
  error_msg: string | null;

  // ---- Common Actions ----
  /** 서비스 상태 변경 (idle, loading, ready) */
  set_status: (status: DevicePoseStatus) => void;
  /** 예외 상황 발생 시 에러 기록 */
  set_error: (error_msg: string) => void;
};

export const useDeviceLocationStore = create<DeviceLocationState>((set) => ({
  is_tracking_location: false,
  location: null,

  status: DevicePoseStatus.IDLE,
  error_msg: null,

  /** 하드웨어에서 전달된 원시 GPS 데이터를 검증하고, 유의미한 변화일 때만 상태를 갱신합니다. */
  set_location: (coords, accuracy, course) =>
    set((state) => {
      const timestamp: f64 = performance.now();
      const prev: DeviceLocation | null = state.location;

      // 1. First-Fix Pass
      if (!prev) {
        return {
          location: { coords, accuracy, course, timestamp },

          status: DevicePoseStatus.READY,
          error_msg: null,
        };
      }

      // 2. Accuracy Guard (Accept if < 100m OR if accuracy is improving)
      const is_improving = accuracy < prev.accuracy;
      if (accuracy > 100 && !is_improving) return state;

      // 3. Deadzone-Filtering (Approx. 1.1m for Lat/Lng, 0.5m for Alt)
      const lng_diff: f64 = Math.abs(prev.coords[0] - coords[0]);
      const lat_diff: f64 = Math.abs(prev.coords[1] - coords[1]);
      const alt_diff: f64 = Math.abs((prev.coords[2] ?? 0) - (coords[2] ?? 0));

      const is_significant_change: boolean = lat_diff > 0.00001 || lng_diff > 0.00001 || alt_diff > 0.5;

      if (!is_significant_change && prev.accuracy === accuracy && prev.course === course) {
        return state;
      }

      /**
       * 4. Update Strategy:
       * Even if the position hasn't moved significantly, we update the metadata (accuracy/course/timestamp)
       * to ensure the UI has the freshest signal information.
       */
      return {
        location: {
          coords: is_significant_change ? coords : prev.coords,
          accuracy,
          course,
          timestamp,
        },

        status: DevicePoseStatus.READY,
        error_msg: null,
      };
    }),

  set_is_tracking_location: (enabled) =>
    set((state) => {
      // 🛡️ 값이 이미 같다면 리렌더링을 발생시키지 않음
      if (state.is_tracking_location === enabled) return state;

      // 추적 중단 시 location 데이터를 null로 초기화하여 stale data 방지
      return {
        is_tracking_location: enabled,
        location: enabled ? state.location : null,
      };
    }),

  /**
   * 서비스 라이프사이클 상태를 안전하게 변경합니다.
   */
  set_status: (status) =>
    set((state) => {
      // 🛡️ 상태가 동일하면 state 참조를 그대로 반환 (리액트 최적화)
      if (state.status === status) return state;
      return { status };
    }),

  /**
   * 하드웨어 접근 실패나 권한 문제 발생 시 에러 상태로 전환합니다.
   */
  set_error: (error_msg) =>
    set((state) => {
      if (state.status === DevicePoseStatus.ERROR && state.error_msg === error_msg) return state;
      return { status: DevicePoseStatus.ERROR, error_msg };
    }),
}));

export const useDeviceOrientationStore = create<DeviceOrientationState>((set) => ({
  orientation: null,
  is_tracking_orientation: false,

  status: DevicePoseStatus.IDLE,
  error_msg: null,

  set_orientation: (heading, beta, gamma) =>
    set((state) => {
      const timestamp: f64 = performance.now();
      const prev: DeviceOrientation | null = state.orientation;

      if (!prev) {
        return {
          orientation: { heading, beta, gamma, timestamp },

          status: DevicePoseStatus.READY, // orientation만 들어와도 서비스 사용 가능 상태로 간주
          error_msg: null,
        };
      }

      // 데드존 필터링 (가장 중요한 heading 기준으로 1.5도 변화 체크)
      const is_heading_changed: boolean = is_delta_over(heading, prev.heading, 1.5);

      // 기울기(Beta, Gamma) 변화 체크 (복셀 엔진에서 화면 흔들림 방지용, 약 2도)
      const is_tilt_changed: boolean = is_delta_over(beta, prev.beta, 2.0) || is_delta_over(gamma, prev.gamma, 2.0);

      if (!is_heading_changed && !is_tilt_changed) return state;

      return {
        orientation: { heading, beta, gamma, timestamp },
      };
    }),

  set_is_tracking_orientation: (enabled) =>
    set((state) => {
      // 🛡️ 값이 이미 같다면 리렌더링을 발생시키지 않음
      if (state.is_tracking_orientation === enabled) return state;

      // 추적 중단 시 orientation 데이터를 null로 초기화
      return {
        is_tracking_orientation: enabled,
        orientation: enabled ? state.orientation : null,
      };
    }),

  /**
   * 서비스 라이프사이클 상태를 안전하게 변경합니다.
   */
  set_status: (status) =>
    set((state) => {
      // 🛡️ 상태가 동일하면 state 참조를 그대로 반환 (리액트 최적화)
      if (state.status === status) return state;
      return { status };
    }),

  /**
   * 하드웨어 접근 실패나 권한 문제 발생 시 에러 상태로 전환합니다.
   */
  set_error: (error_msg) =>
    set((state) => {
      if (state.status === DevicePoseStatus.ERROR && state.error_msg === error_msg) return state;
      return { status: DevicePoseStatus.ERROR, error_msg };
    }),
}));
