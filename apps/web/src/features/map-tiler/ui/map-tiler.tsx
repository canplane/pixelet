// 1. 외부 라이브러리 (External Libraries)
import { useEffect, useRef } from 'react';
import maplibregl from 'maplibre-gl';

// 2. 프로젝트 공통 자산 및 타입 (Shared)
import type { Location } from '@/shared/types/map';

// 3. 엔티티 및 상태 관리 (Entities)
import { useDeviceLocationStore } from '@/entities/device-location/model/device-location.store';

// 4. 기능 전용 설정 (Feature Config)
import { mapStyle } from '../config/map-style';

const DEFAULT_LOCATION: Location = { lng: 126.978, lat: 37.5665 }; // Seoul

/**
 * MapLibre 기반의 2D 지도 테스트 컴포넌트입니다.
 * 기기의 실제 위치를 추적하여 지도의 중심을 이동시킵니다.
 */
export function MapTiler() {
  const { location: deviceLocation, status } = useDeviceLocationStore();

  const mapContainerRef = useRef<HTMLDivElement | null>(null);
  const mapRef = useRef<maplibregl.Map | null>(null);

  /**
   * 지도 인스턴스 초기화
   */
  useEffect(() => {
    if (!mapContainerRef.current || mapRef.current) return;

    const map = new maplibregl.Map({
      container: mapContainerRef.current,
      style: mapStyle.local(),
      center: [DEFAULT_LOCATION.lng, DEFAULT_LOCATION.lat], // MapLibre는 [lng, lat] 배열을 사용
      zoom: 11,
    });

    map.addControl(new maplibregl.NavigationControl(), 'top-right');
    mapRef.current = map;

    return () => {
      map.remove();
      mapRef.current = null;
    };
  }, []);

  /**
   * 기기 위치 변경 시 지도 중심 업데이트
   */
  useEffect(() => {
    if (!mapRef.current || !deviceLocation || status !== 'ready') return;

    mapRef.current.easeTo({
      center: [deviceLocation.lng, deviceLocation.lat],
      duration: 500,
    });
  }, [deviceLocation, status]);

  return (
    <div className="fixed inset-0">
      <div ref={mapContainerRef} className="w-full h-full" />
    </div>
  );
}
