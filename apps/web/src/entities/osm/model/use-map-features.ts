import { useState, useEffect, useCallback, useRef } from 'react';
import { fetchOsmData } from '../api/overpass.api';
import { MAP_QUERIES } from '../lib/overpass-queries';
import type { OverpassElement, OverpassResponse } from '../model/types';
import type { LngLat } from '@/shared/types/map';
import { OSM_CONFIG } from '../config/osm-config';

/**
 * 하버사인(Haversine) 공식을 이용한 두 위경도 사이의 직선 거리(m) 계산
 */
function getDistance(pos1: LngLat, pos2: LngLat): number {
  const R = 6371e3; // 지구 반지름 (meters)
  const dLat = ((pos2[1] - pos1[1]) * Math.PI) / 180;
  const dLon = ((pos2[0] - pos1[0]) * Math.PI) / 180;
  const a =
    Math.sin(dLat / 2) * Math.sin(dLat / 2) +
    Math.cos((pos1[1] * Math.PI) / 180) *
      Math.cos((pos2[1] * Math.PI) / 180) *
      Math.sin(dLon / 2) *
      Math.sin(dLon / 2);
  const c = 2 * Math.atan2(Math.sqrt(a), Math.sqrt(1 - a));
  return R * c;
}

export function useMapFeatures(
  queryName: keyof typeof MAP_QUERIES,
  lngLat: LngLat | null,
  radius: number = OSM_CONFIG.DEFAULT_RADIUS,
) {
  // 1. 초기값을 null이 아닌 빈 배열 객체로 설정 (에러 방지)
  const [osmData, setOsmData] = useState<OverpassResponse>({ elements: [] });
  const [loading, setLoading] = useState<boolean>(false);
  const [error, setError] = useState<Error | null>(null);

  const [canRequest, setCanRequest] = useState<boolean>(true);
  const lastRequestTimeRef = useRef<number>(0);
  const lastRequestPosRef = useRef<LngLat | null>(null);

  // [보강] 데이터 병합 및 거리 기반 청소 로직
  const mergeAndCleanup = useCallback(
    (newElements: OverpassElement[], currentCenter: LngLat) => {
      setOsmData((prev) => {
        // 1. 기존 데이터 + 신규 데이터 합치기
        const combined = [...prev.elements, ...newElements];

        // 2. ID 중복 제거 및 거리 기반 필터링 (CLEANUP_THRESHOLD 기준)
        const uniqueAndNearby = combined.filter((el, index, self) => {
          // 중복 체크
          const isFirst = index === self.findIndex((t) => t.id === el.id);
          if (!isFirst) return false;

          // 거리 체크용 좌표 추출 (center 우선, 없으면 geometry[0])
          const coord = el.center
            ? [el.center.lon, el.center.lat]
            : el.geometry?.[0]
              ? [el.geometry[0].lon, el.geometry[0].lat]
              : null;

          if (coord) {
            const dist = getDistance(coord as LngLat, currentCenter);
            const isTooFar = dist > OSM_CONFIG.CLEANUP_THRESHOLD;
            return !isTooFar; // 너무 멀면 버리고(false), 가까우면 유지(true)
          }

          return true;
        });

        return { elements: uniqueAndNearby };
      });
    },
    [],
  );

  useEffect(() => {
    // MapScene의 fetchCenter가 들어올 때까지 대기
    if (!lngLat || !canRequest) return;

    const now = Date.now();
    const timeSinceLastRequest = now - lastRequestTimeRef.current;

    if (lastRequestPosRef.current) {
      const movedDistance = getDistance(lastRequestPosRef.current, lngLat);
      if (movedDistance < OSM_CONFIG.DISTANCE_THRESHOLD) return;
    }

    if (timeSinceLastRequest < OSM_CONFIG.REQUEST_INTERVAL) return;

    const loadData = async () => {
      const query: string = MAP_QUERIES[queryName]({ lngLat, radius });

      setLoading(true);
      setCanRequest(false);
      lastRequestTimeRef.current = Date.now();
      lastRequestPosRef.current = lngLat;

      try {
        const data: OverpassResponse = await fetchOsmData(query);
        // [수정 포인트] mergeAndCleanup을 호출하여 데이터 누적 및 먼 데이터 삭제 실행
        if (data.elements) {
          mergeAndCleanup(data.elements, lngLat);
        }
        setError(null);
      } catch (err) {
        console.error('[OSM] 데이터 요청 실패:', err);
        setError(err instanceof Error ? err : new Error('Unknown error'));
      } finally {
        setLoading(false);
        setTimeout(() => setCanRequest(true), OSM_CONFIG.REQUEST_INTERVAL);
      }
    };

    loadData();
  }, [lngLat, radius, canRequest, queryName, mergeAndCleanup]);

  return {
    osmData,
    loading,
    error,
    isWaiting: !canRequest && !loading,
  };
}
