import { useMemo, useState, useEffect } from 'react';
import { debounce } from 'lodash';
import { useMapStore } from '../model/map-store';
import { useMapFeatures } from '@/entities/osm/model/use-map-features';
import type { Location } from '@/shared/types/map';

export function useOsmDataFetcher() {
  const setStatus = useMapStore((state) => state.setStatus);
  const setLoadingStep = useMapStore((state) => state.setLoadingStep);

  const center = useMapStore((state) => state.center);
  const unproject = useMapStore((state) => state.unproject);

  // 1. 현재 중심점의 위경도 계산
  const currentViewLocation = useMemo(() => {
    if (!center) return null;
    return unproject(center);
  }, [center, unproject]);

  // 2. 디바운스된 위치 상태 (API 호출용)
  const [fetchLocation, setFetchLocation] = useState<Location | null>(null);
  // 디바운스 처리: 사용자가 지도를 멈추고 800ms 후에만 fetchLocation을 갱신
  const debouncedSetFetch = useMemo(
    () => debounce((loc: Location) => setFetchLocation(loc), 800),
    [],
  );

  useEffect(() => {
    if (currentViewLocation) debouncedSetFetch(currentViewLocation);
    // 언마운트 시 예약된 작업 취소
    return () => debouncedSetFetch.cancel();
  }, [currentViewLocation]);

  // 3. OSM 데이터 패칭
  const { data, isLoading, error } = useMapFeatures(
    ['GEOMETRY_ROADS_MAJOR', 'POI_MAJOR'],
    fetchLocation,
  );

  // 로딩 상태 동기화
  useEffect(() => {
    // fetchLocation이 없으면 아직 시작도 안 한 상태이므로 status를 건드리지 않거나 idle 유지
    if (!fetchLocation) return;

    if (isLoading) {
      setStatus('loading');

      // 🚀 데이터의 특성을 파악해 단계를 나눕니다.
      // Overpass 데이터에서 'highway'가 있으면 도로, 'amenity/shop' 등이 있으면 POI로 간주합니다.
      const hasRoads = data.elements.some((el) => el.tags?.highway);
      const hasPOI = data.elements.some(
        (el) => el.tags?.amenity || el.tags?.shop || el.tags?.tourism,
      );

      if (hasRoads && !hasPOI) {
        setLoadingStep('poi');
      } else if (!hasRoads) {
        setLoadingStep('roads');
      } else {
        setLoadingStep('complete');
      }
    } else if (error) {
      setStatus('error');
    } else if (data) {
      // 🚀 데이터가 모두 들어왔을 때만 success로 전환
      // 만약 osmData가 객체 형태(예: { GEOMETRY_ROADS_ONLY: [...], POI: [...] })라면
      // 모든 키에 데이터가 있는지 확인하는 로직을 추가하면 더 정확합니다.
      setStatus('success');
    }
  }, [isLoading, error, data, setStatus]);

  return { data }; // 이제 상태는 스토어에서 관리하므로 데이터만 반환
}
