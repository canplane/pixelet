// 1. 외부 라이브러리 (External Libraries)
import { useState, useEffect, useCallback, useRef } from 'react';

// 2. 프로젝트 공통 타입 및 설정 (Shared)
import type { Location } from '@/shared/types/map';
import { OSM_CONFIG } from '../config/osm-config';

// 3. API 및 도메인 로직 (Entities / Features)
import { requestOverpass } from '../api/overpass.api';
import { MAP_QUERIES } from '../lib/overpass-queries';
import type { OverpassElement, OverpassResponse } from '../model/types';

/**
 * 하버사인(Haversine) 공식을 이용한 두 위경도 사이의 직선 거리(m) 계산
 */
function getDistance(pos1: Location, pos2: Location): number {
  const R = 6371e3; // 지구 반지름 (m)
  const dLat = ((pos2.lat - pos1.lat) * Math.PI) / 180;
  const dLon = ((pos2.lng - pos1.lng) * Math.PI) / 180;
  const a =
    Math.sin(dLat / 2) * Math.sin(dLat / 2) +
    Math.cos((pos1.lat * Math.PI) / 180) *
      Math.cos((pos2.lat * Math.PI) / 180) *
      Math.sin(dLon / 2) *
      Math.sin(dLon / 2);
  const c = 2 * Math.atan2(Math.sqrt(a), Math.sqrt(1 - a));
  return R * c;
}

/**
 * OSM 데이터를 관리하고 실시간 위치 변화에 따라 데이터를 점진적으로 로드하는 훅입니다.
 * - 거리 기반 요청 제어 (Distance Threshold)
 * - 시간 기반 요청 제어 (Throttle)
 * - 거리 기반 메모리 정리 (Cleanup)
 */
export function useMapFeatures(
  queryName: keyof typeof MAP_QUERIES,
  location: Location | null,
  radius: number = OSM_CONFIG.DEFAULT_RADIUS,
) {
  const [osmData, setOsmData] = useState<OverpassResponse>({ elements: [] });
  const [isLoading, setIsLoading] = useState<boolean>(false);
  const [error, setError] = useState<Error | null>(null);

  const [canRequest, setCanRequest] = useState<boolean>(true);
  const lastRequestTimeRef = useRef<number>(0);
  const lastRequestPosRef = useRef<Location | null>(null);

  /**
   * [데이터 병합 및 메모리 관리]
   * 중복을 제거하고 현재 위치에서 임계치 이상 멀어진 데이터를 제거합니다.
   */
  const mergeAndCleanup = useCallback(
    (newElements: OverpassElement[], currentCenter: Location) => {
      setOsmData((prev) => {
        const combined = [...prev.elements, ...newElements];

        const uniqueAndNearby = combined.filter((el, index, self) => {
          // 1. ID 기반 중복 제거
          const isFirst = index === self.findIndex((t) => t.id === el.id);
          if (!isFirst) return false;

          // 2. 현재 중심점으로부터의 거리 계산
          const lat = el.center?.lat ?? el.lat ?? el.geometry?.[0]?.lat;
          const lon = el.center?.lon ?? el.lon ?? el.geometry?.[0]?.lon;

          if (lat !== undefined && lon !== undefined) {
            const dist = getDistance({ lat, lng: lon }, currentCenter);
            return dist <= OSM_CONFIG.CLEANUP_THRESHOLD;
          }

          return true;
        });

        return { elements: uniqueAndNearby };
      });
    },
    [],
  );

  useEffect(() => {
    if (!location || !canRequest) return;

    // A. 거리 기반 제어: 최소 이동 거리 검증
    if (lastRequestPosRef.current) {
      const movedDistance = getDistance(lastRequestPosRef.current, location);
      if (movedDistance < OSM_CONFIG.DISTANCE_THRESHOLD) return;
    }

    // B. 시간 기반 제어: 최소 요청 간격 검증
    const now = Date.now();
    if (now - lastRequestTimeRef.current < OSM_CONFIG.REQUEST_INTERVAL) return;

    const loadData = async () => {
      // 이전에 정의한 쿼리 매개변수 구조 { location, radius } 반영
      const query = MAP_QUERIES[queryName]({ location, radius });

      setIsLoading(true);
      setCanRequest(false);
      lastRequestTimeRef.current = now;
      lastRequestPosRef.current = location;

      try {
        const data = await requestOverpass(query);
        if (data.elements) {
          mergeAndCleanup(data.elements, location);
        }
        setError(null);
      } catch (err) {
        setError(
          err instanceof Error ? err : new Error('Overpass request failed'),
        );
      } finally {
        setIsLoading(false);
        // 인터벌 시간이 흐른 뒤 다시 요청이 가능하도록 락 해제
        setTimeout(() => setCanRequest(true), OSM_CONFIG.REQUEST_INTERVAL);
      }
    };

    loadData();
  }, [location, radius, canRequest, queryName, mergeAndCleanup]);

  return { osmData, isLoading, error, isWaiting: !canRequest && !isLoading };
}
