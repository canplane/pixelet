import { type ReactNode } from 'react';
import type { Coord2D } from '@pixelet/core/shared/model/coords';
import { useTerrainAltitude } from '@/entities/geo/lib/use-terrain-altitude';
import { FloatingGroup } from '@/entities/geo/ui/floating-group';

interface TerrainStickyGroupProps {
  /** 객체의 절대 좌표 (Global Coordinate) */
  coord: Coord2D;
  /** 바닥에서의 높이 보정값 (선택) */
  offset?: number;
  children: ReactNode;
}

export function TerrainStickyGroup({
  coord,
  offset = 0,
  children,
}: TerrainStickyGroupProps) {
  const { getTerrainAltitude } = useTerrainAltitude();

  // 여기서 지형 높이를 조회합니다.
  const groundHeight = getTerrainAltitude(coord) ?? 0;
  const absoluteAltitude = groundHeight + offset;

  // 계산된 '절대 높이'를 FloatingGroup에게 넘깁니다.
  return (
    <FloatingGroup coord={{ ...coord, z: absoluteAltitude }}>
      {children}
    </FloatingGroup>
  );
}
