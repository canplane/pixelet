import { useEffect } from 'react';
import { useMapNavigation } from './use-map-navigation';

/**
 * Follow Mode(추적 모드) 활성화 시
 * 사용자의 위치와 방향을 지도에 실시간 동기화하는 엔진 훅입니다.
 */
export function useMapFollowMode() {
  const {
    setCenter,
    setViewState, // 기존에 이미 잘 만들어두신 액션 활용
    devicePos,
    compassHeading,
    isFollowMode,
  } = useMapNavigation();

  useEffect(() => {
    // 1. 추적 모드가 꺼져있거나 센서 데이터가 없으면 중단
    if (!isFollowMode || !devicePos || compassHeading === null) {
      return;
    }

    // 2. 위치 동기화: 위경도 -> 3D 좌표 변환
    setCenter(devicePos);

    // 3. 시선 동기화: 나침반 각도를 지도의 bearing으로 업데이트
    // viewState 전체를 건드리지 않고 bearing만 덮어씌웁니다.
    setViewState({ bearing: compassHeading });
  }, [isFollowMode, devicePos, compassHeading, setCenter, setViewState]);
}
