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
    set({
      location: data.location,
      accuracy: data.accuracy,
      gpsHeading: data.heading,
      status: 'ready',
      message: null, // 데이터 수신 성공 시 메시지 초기화
    }),

  setCompassHeading: (heading) => set({ compassHeading: heading }),

  // 상태만 변경할 때 사용 (예: 처음 로딩 시작 시)
  setStatus: (status) => set({ status }),

  // 에러 발생 시 상태와 메시지를 동시에 업데이트
  setError: (message) => set({ status: 'error', message }),
}));
