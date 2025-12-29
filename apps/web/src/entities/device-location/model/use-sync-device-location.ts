import { useDeviceGeolocation } from './use-device-geolocation';
import { useDeviceOrientation } from './use-device-orientation';

/**
 * [use-sync-device-location]
 * 기기의 모든 물리 센서(GPS, 나침반)를 활성화하고 전역 스토어와 동기화합니다.
 */
export function useSyncDeviceLocation() {
  // 1. GPS 추적 시작 (내부에서 스토어에 직접 setGpsData, setError 호출함)
  useDeviceGeolocation();

  // 2. 나침반 센서 가동 및 제어권 획득
  const { requestPermission, isPermissionGranted } = useDeviceOrientation();

  // 3. UI 컨트롤에 필요한 기능만 노출
  return {
    requestOrientation: requestPermission,
    isOrientationGranted: isPermissionGranted,
  };
}
