// @pxl/web/modules/map-tiler/map-tiler.tsx

import { useEffect, useRef } from "react";
import maplibregl from "maplibre-gl";

import type { G4326 } from "@pixelet/shared/spec/space/adapter/4326.ts";

import { type DeviceLocation, DevicePoseStatus, useDeviceLocationStore } from "@/modules/device/pose/store.ts";
import { mapStyle } from "./config/style.ts";

const DEFAULT_LOCATION: G4326 = [126.978, 37.5665]; // Seoul

/**
 * MapLibre 기반의 2D 지도 테스트 컴포넌트입니다.
 * 기기의 실제 위치를 추적하여 지도의 중심을 이동시킵니다.
 */
export function MapTiler() {
  const location_status: DevicePoseStatus = useDeviceLocationStore((s) => s.status);
  const location: DeviceLocation | null = useDeviceLocationStore((s) => s.location);

  const map_container_ref = useRef<HTMLDivElement | null>(null);
  const map_ref = useRef<maplibregl.Map | null>(null);

  /**
   * 지도 인스턴스 초기화
   */
  useEffect(() => {
    if (!map_container_ref.current || map_ref.current) return;

    const map = new maplibregl.Map({
      container: map_container_ref.current,
      style: mapStyle.local(),
      center: [DEFAULT_LOCATION[0], DEFAULT_LOCATION[1]], // MapLibre는 [lng, lat] 배열을 사용
      zoom: 11,
    });

    map.addControl(new maplibregl.NavigationControl(), "top-right");
    map_ref.current = map;

    return () => {
      map.remove();
      map_ref.current = null;
    };
  }, []);

  /**
   * 기기 위치 변경 시 지도 중심 업데이트
   */
  useEffect(() => {
    if (!map_ref.current || !location || location_status !== DevicePoseStatus.READY) return;

    map_ref.current.easeTo({
      center: [location.coords[0], location.coords[1]],
      duration: 500,
    });
  }, [location, location_status]);

  return (
    <div className="fixed inset-0">
      <div ref={map_container_ref} className="w-full h-full" />
    </div>
  );
}
