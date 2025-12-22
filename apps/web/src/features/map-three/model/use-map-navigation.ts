import { useState, useEffect, useMemo } from 'react';
import { useShallow } from 'zustand/react/shallow';
import { useMapStore } from './map-store';
import { useDeviceLocationStore } from '@/entities/device-location/model/device-location.store';

/**
 * 지도 내비게이션 및 위치 제어에 필요한 상태와 액션들을
 * 성능 최적화(Shallow) 처리하여 반환하는 커스텀 셀렉터 훅입니다.
 */
export function useMapNavigation() {
  // 1. 외부 스토어의 위치 정보는 최상단에서 구독
  const deviceLocation = useDeviceLocationStore((state) => state.location);
  const compassHeading = useDeviceLocationStore(
    (state) => state.compassHeading,
  );

  // 1. Store에서 필요한 액션과 상태만 가져옵니다.
  const store = useMapStore(
    useShallow((state) => ({
      center: state.center,
      setCenter: state.setCenter,
      viewState: state.viewState, // 묶음으로 가져오기
      setViewState: state.setViewState, // bearing 등을 수정하기 위해 필요
      origin: state.origin,
      setOrigin: state.setOrigin,
      isFollowMode: state.isFollowMode,
      setIsFollowMode: state.setIsFollowMode,
      isInteracting: state.isInteracting,
      setIsInteracting: state.setIsInteracting,
    })),
  );
  const project = useMapStore((state) => state.project);

  // 2. devicePos와 isOffCenter를 훅 내부에서 useMemo로 관리합니다.
  // 이렇게 하면 deviceLocation이나 center가 "유의미하게" 바뀔 때만 리렌더링을 유발합니다.
  const devicePos = useMemo(
    () => (deviceLocation ? project(deviceLocation) : null),
    [deviceLocation, project],
  );

  const [isOffCenter, setIsOffCenter] = useState(false);

  /**
   * [Hysteresis Logic]
   * 목적: '이탈 판정'과 '복귀 판정'의 기준선을 다르게 설정하여 UI 상태가 급격하게 변하는 것을 막습니다.
   * 적용:
   * - 진입(Off-Center): 중심에서 10m 이상 멀어져야 이탈로 간주 (확실한 탐색 의도 확인)
   * - 해제(On-Center): 중심에서 3m 이내로 들어와야 복귀로 간주 (확실한 위치 정렬 확인)
   * 결과: 기준선 근처에서 위치가 미세하게 변해도 아이콘이 깜빡거리거나 애니메이션이 반복되지 않습니다.
   */
  useEffect(() => {
    if (!devicePos || !store.center) return;

    const dist = Math.sqrt(
      Math.pow(devicePos.x - store.center.x, 2) +
        Math.pow(devicePos.z - store.center.z, 2),
    );

    // 🛡️ UI 상태 전환에만 히스테리시스 적용
    if (!isOffCenter && dist > 10) {
      setIsOffCenter(true); // 확실히 멀어졌을 때만 이탈
    } else if (isOffCenter && dist < 3) {
      setIsOffCenter(false); // 확실히 가까워졌을 때만 복귀
    }
  }, [devicePos, store.center, isOffCenter]);

  return {
    ...store,
    devicePos, // 핸들러에서 사용하기 위해 포함
    isOffCenter,
    deviceLocation, // 원본 위경도가 필요할 수 있으므로 포함
    compassHeading,
  };
}
