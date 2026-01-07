import { useMemo } from 'react';
import { useMapNavigation } from '@/entities/geo/model/use-map-navigation';
import { useTerrainAltitude } from '@/entities/geo/lib/use-terrain-altitude';

/**
 * 🎯 [Bridge] 지표면 고도(Z)가 포함된 절대적인 지도 중심 좌표를 제공합니다.
 */
export function useMapCenter() {
  const { center } = useMapNavigation(); // x, y (3857)
  const { getTerrainAltitude } = useTerrainAltitude(); // 지형 고도 계산 로직

  // 🎯 Z값이 포함된 무결한 센터 좌표 계산
  const absoluteCenter = useMemo(() => {
    const terrainZ = getTerrainAltitude({ x: center.x, y: center.y }) ?? 0;

    return {
      x: center.x,
      y: center.y,
      z: terrainZ, // 지표면에 Stick된 실시간 고도
    };
  }, [center.x, center.y, getTerrainAltitude]);

  return absoluteCenter;
}
