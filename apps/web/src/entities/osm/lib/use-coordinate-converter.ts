import { useCallback, useMemo } from 'react';
import { useMapStore } from '../model/map-store';
import { CoordOps, type Coord } from '@pixelet/shared/shared/model/coords';

/**
 * [use-coordinate-converter] (Base Hook)
 * Floating Origin 시스템의 핵심인 변환 함수들을 제공합니다.
 */
export function useCoordinateConverter() {
  const origin = useMapStore((state) => state.origin);

  /** 🎯 Local (Scene) -> Global (World) */
  const localToGlobal = useCallback(
    (localPos: Coord): Coord => CoordOps.add(localPos, origin),
    [origin],
  );

  /** 🎯 Global (World) -> Local (Scene) */
  const globalToLocal = useCallback(
    (globalPos: Coord): Coord => CoordOps.sub(globalPos, origin),
    [origin],
  );

  return { origin, localToGlobal, globalToLocal };
}

/**
 * [use-relative-position] (Derived Hook)
 * 절대 좌표를 현재 씬의 로컬 좌표로 변환하여 메모이제이션합니다. (Global -> Local)
 */
export function useRelativePosition(absolutePos: Coord): Coord {
  const { globalToLocal } = useCoordinateConverter();

  return useMemo(
    () => globalToLocal(absolutePos),
    [globalToLocal, absolutePos],
  );
}

/**
 * [use-absolute-position] (Derived Hook / New 🚀)
 * 씬 내부의 로컬 객체 좌표를 실제 세계의 절대 좌표로 복원합니다. (Local -> Global)
 * * Usage: 드래그 중인 객체의 실제 지도 좌표를 실시간으로 표시할 때 유용합니다.
 */
export function useAbsolutePosition(localPos: Coord): Coord {
  const { localToGlobal } = useCoordinateConverter();

  return useMemo(() => localToGlobal(localPos), [localToGlobal, localPos]);
}
