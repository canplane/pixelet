import { useEffect } from 'react';
import { useGeolocation } from './use-geolocation';
import { useDeviceOrientation } from './use-orientation';
import { useDeviceLocationStore } from './device-location.store';

export function useSyncLocation() {
  // 1. GPS 추적 시작 (내부에서 스토어에 직접 setGpsData, setError 호출함)
  useGeolocation();

  // 2. 나침반 센서 가동
  const { compassHeading } = useDeviceOrientation();
  const { setCompassHeading } = useDeviceLocationStore();

  // 3. 나침반 정보만 동기화
  // compassHeading은 빈번하게 변하므로 별도의 useEffect로 관리합니다.
  useEffect(() => {
    setCompassHeading(compassHeading);
  }, [compassHeading, setCompassHeading]);
}
