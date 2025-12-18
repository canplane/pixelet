// src/entities/device-location/model/use-sync-location.ts
import { useEffect } from 'react';
import { useGeolocation } from './use-geolocation';
import { useDeviceOrientation } from './use-orientation';
import { useDeviceLocationStore } from './device-location.store';

export function useSyncLocation() {
  const geo = useGeolocation();
  const { compassHeading } = useDeviceOrientation();
  const { setGpsData, setCompassHeading } = useDeviceLocationStore();

  // 1. GPS 정보 동기화
  useEffect(() => {
    if (geo.status === 'ready') {
      setGpsData({
        lngLat: geo.lngLat,
        accuracy: geo.accuracy,
        heading: geo.heading, // 이동 방향
      });
    }
  }, [geo, setGpsData]);

  // 2. 나침반 정보 동기화
  useEffect(() => {
    setCompassHeading(compassHeading);
  }, [compassHeading, setCompassHeading]);
}
