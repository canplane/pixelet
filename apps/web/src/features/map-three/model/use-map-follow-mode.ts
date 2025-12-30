import { useEffect } from 'react';
import { useMapNavigation } from '../../../entities/osm/model/use-map-navigation';

/**
 * Follow Mode(추적 모드) 활성화 시
 * 사용자의 위치와 방향을 지도에 실시간 동기화하는 엔진 훅입니다.
 */
export function useMapFollowMode() {
  const { setCenter, setViewState, devicePos, compassHeading, isFollowMode } =
    useMapNavigation();

  // 1. [위치 동기화] GPS 좌표가 변경될 때만 실행
  useEffect(() => {
    if (!isFollowMode || !devicePos) return;

    // 🚀 단순히 좌표만 업데이트 (회전값 건드리지 않음)
    setCenter(devicePos);
  }, [isFollowMode, devicePos, setCenter]);

  // 2. [시선 동기화] 나침반 각도가 변경될 때만 실행
  useEffect(() => {
    if (!isFollowMode || compassHeading === null) return;

    // 🚀 단순히 회전각만 업데이트 (좌표 건드리지 않음)
    setViewState({ bearing: compassHeading });
  }, [isFollowMode, compassHeading, setViewState]);
}
