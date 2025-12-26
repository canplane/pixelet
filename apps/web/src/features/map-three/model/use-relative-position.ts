import { useMemo } from 'react';
import { useMapStore } from './map-store';
import type { MeterPos } from '@pixelet/shared/shared/model/coords';

/**
 * [use-relative-position]
 * 절대 미터 좌표를 현재 Floating Origin 기반의 상대 좌표(MeterPos)로 변환합니다.
 * 반환 타입을 객체로 통일하여 좌표 연산의 일관성을 유지합니다.
 */
export function useRelativePosition(absolutePos: MeterPos): MeterPos {
  const originOffset = useMapStore((state) => state.originOffset);

  return useMemo(
    () => ({
      x: absolutePos.x - originOffset.x,
      y: absolutePos.y - originOffset.y,
      z: absolutePos.z - originOffset.z,
    }),
    [
      absolutePos.x,
      absolutePos.y,
      absolutePos.z,
      originOffset.x,
      originOffset.y,
      originOffset.z,
    ],
  );
}
