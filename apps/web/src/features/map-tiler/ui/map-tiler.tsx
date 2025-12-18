// Uses OpenMapLibre library

import { useEffect, useRef } from 'react';
import maplibregl from 'maplibre-gl';

import { useDeviceLocationStore } from '@/entities/device-location/model/device-location.store';
import { mapStyle } from '../config/map-style';
import type { LngLat } from '@/shared/types/map';

const DEFAULT_LNGLAT: LngLat = [126.978, 37.5665]; // Seoul

/** Test MapTiler */
export function MapTiler() {
  const { lngLat: deviceLngLat, status } = useDeviceLocationStore();

  const mapContainerRef = useRef<HTMLDivElement | null>(null);
  const mapRef = useRef<maplibregl.Map | null>(null);

  useEffect(() => {}, []);

  /* Initialize map instance */
  useEffect(() => {
    if (!mapContainerRef.current) return;
    if (mapRef.current) return;

    const map = new maplibregl.Map({
      container: mapContainerRef.current,
      style: mapStyle.local(),
      center: DEFAULT_LNGLAT, // fallback
      zoom: 11,
    });

    map.addControl(new maplibregl.NavigationControl(), 'top-right');

    mapRef.current = map;

    return () => {
      map.remove();
      mapRef.current = null;
    };
  }, []);

  /* Update map center on location change */
  useEffect(() => {
    if (!mapRef.current) return;
    if (!deviceLngLat) return;
    if (status !== 'ready') return;

    mapRef.current.easeTo({
      center: deviceLngLat,
      duration: 500,
    });
  }, [deviceLngLat, status]);

  return (
    <div className="fixed inset-0">
      <div ref={mapContainerRef} className="w-full h-full" />
    </div>
  );
}
