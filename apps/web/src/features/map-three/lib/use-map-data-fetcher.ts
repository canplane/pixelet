import { useMemo, useState, useEffect } from 'react';
import { debounce } from 'lodash';
import { useMapStore } from '../model/map-store';
import { useMapFeatures } from '@/entities/osm/model/use-map-features';
import type { Location } from '@/shared/types/map';

export function useMapDataFetcher() {
  const setStatus = useMapStore((state) => state.setStatus);
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
  const { osmData, isLoading, error } = useMapFeatures(
    'GEOMETRY_ROADS_ONLY',
    fetchLocation,
  );

  // 로딩 상태 동기화
  useEffect(() => {
    if (isLoading) setStatus('loading');
    else if (error) setStatus('error');
    else if (osmData) setStatus('success');
  }, [isLoading, error, osmData, setStatus]);

  return { osmData }; // 이제 상태는 스토어에서 관리하므로 데이터만 반환
}
