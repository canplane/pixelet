import { useCallback, useMemo } from 'react';
import { useMapStore } from '../model/map-store';
import type { MeterPos } from '@pixelet/shared/shared/model/coords';

/**
 * [use-relative-position] (Value Hook)
 * 절대 미터 좌표(Absolute)를 렌더링을 위한 로컬 좌표(Local)로 변환합니다.
 * * Formula: Local = Absolute - Origin
 * * Usage: 렌더링 루프(Render Loop)나 컴포넌트 프로퍼티 계산 시 사용
 */
export function useRelativePosition(absolutePos: MeterPos): MeterPos {
  const origin = useMapStore((state) => state.origin);

  return useMemo(
    () => ({
      x: absolutePos.x - origin.x,
      y: absolutePos.y - origin.y,
      z: absolutePos.z - origin.z,
    }),
    [absolutePos.x, absolutePos.y, absolutePos.z, origin.x, origin.y, origin.z],
  );
}

/**
 * [use-coordinate-converter] (Callback Hook)
 * 이벤트 핸들러 등 리액트 렌더링 사이클 밖에서 좌표 변환이 필요할 때 사용하는 함수들을 제공합니다.
 * * Usage: onClick, onEnd 등의 이벤트 콜백 내부
 */
export function useCoordinateConverter() {
  const origin = useMapStore((state) => state.origin);

  /**
   * [local-to-global]
   * Three.js 씬의 로컬 좌표를 실제 지리적 절대 좌표로 복원합니다.
   */
  const localToGlobal = useCallback(
    (localPos: MeterPos): MeterPos => {
      return {
        x: localPos.x + origin.x,
        y: localPos.y + origin.y,
        z: localPos.z + origin.z,
      };
    },
    [origin],
  );

  /**
   * [global-to-local]
   * 절대 좌표를 현재 오리진 기준의 로컬 좌표로 변환합니다.
   */
  const globalToLocal = useCallback(
    (globalPos: MeterPos): MeterPos => {
      return {
        x: globalPos.x - origin.x,
        y: globalPos.y - origin.y,
        z: globalPos.z - origin.z,
      };
    },
    [origin],
  );

  return { localToGlobal, globalToLocal };
}
