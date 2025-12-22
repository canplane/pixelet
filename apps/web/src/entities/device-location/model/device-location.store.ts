import { create } from 'zustand';
import type { Location } from '@/shared/types/map';

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
       * [Deadzone Filtering]
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
        message: null, // 데이터 수신 성공 시 메시지 초기화
      };
    }),

  setCompassHeading: (heading) => set({ compassHeading: heading }),

  // 상태만 변경할 때 사용 (예: 처음 로딩 시작 시)
  setStatus: (status) => set({ status }),

  // 에러 발생 시 상태와 메시지를 동시에 업데이트
  setError: (message) => set({ status: 'error', message }),
}));
