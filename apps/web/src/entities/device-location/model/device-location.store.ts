import { create } from 'zustand';
import type { Location } from '@/shared/types/map';

/**
 * [device-location-store]
 * 기기의 물리적 센서(GPS, 나침반) 데이터와 수신 상태를 관리하는 스토어입니다.
 */

export type DeviceLocationStatus = 'idle' | 'loading' | 'ready' | 'error';

interface DeviceLocationState {
  /** 기기의 현재 위경도 및 고도 */
  location: Location | null;
  /** GPS 정확도 (단위: 미터) */
  accuracy: number | null;
  /** 이동 방향 (GPS 기준, 0~360도) */
  gpsHeading: number | null;
  /** 시선 방향 (나침반 센서 기준, 0~360도) */
  compassHeading: number | null;
  /** 위치 권한 및 수신 상태 */
  status: DeviceLocationStatus;
  /** 에러 또는 상태 관련 메시지 */
  message: string | null;

  /** GPS 데이터 업데이트 */
  setGpsData: (data: {
    location: Location;
    accuracy: number;
    heading: number | null;
  }) => void;

  /** 나침반 센서 데이터 업데이트 */
  setCompassHeading: (heading: number | null) => void;

  /** 상태 변경 (loading 등) */
  setStatus: (status: DeviceLocationStatus) => void;

  /** 에러 발생 시 호출 */
  setError: (message: string) => void;
}

export const useDeviceLocationStore = create<DeviceLocationState>((set) => ({
  location: null,
  accuracy: null,
  gpsHeading: null,
  compassHeading: null,
  status: 'idle',
  message: null,

  /**
   * [set-gps-data]
   * 위경도 데이터 수신 시 유효성 검사 및 필터링을 거쳐 상태를 업데이트합니다.
   */
  setGpsData: (data) =>
    set((state) => {
      const prevLoc = state.location;
      const newLoc = data.location;

      // 초기 데이터인 경우 즉시 업데이트
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
       * [deadzone-filtering]
       * 목적: GPS 센서 특유의 미세한 떨림(Jitter)이 앱 전체로 전파되는 것을 차단합니다.
       * 기준: 약 1.1m(소수점 5자리) 미만의 변화는 무시합니다.
       * 결과: 실제 유의미한 이동이 없을 경우 Store 상태를 갱신하지 않아 불필요한 리렌더링을 방지합니다.
       */
      const latDiff = Math.abs(prevLoc.lat - newLoc.lat);
      const lngDiff = Math.abs(prevLoc.lng - newLoc.lng);

      const isSignificantChange = latDiff > 0.00001 || lngDiff > 0.00001;

      // 필터링된 결과 반영
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
   * 마그네토미터 센서에서 얻은 기기 전면의 절대 방위각을 업데이트합니다.
   */
  setCompassHeading: (heading) => set({ compassHeading: heading }),

  /**
   * [set-status]
   * 위치 권한 요청 및 데이터 수신 생명주기 상태를 변경합니다.
   */
  setStatus: (status) => set({ status }),

  /**
   * [set-error]
   * 하드웨어 접근 실패나 권한 거부 시 에러 메시지를 기록합니다.
   */
  setError: (message) => set({ status: 'error', message }),
}));
