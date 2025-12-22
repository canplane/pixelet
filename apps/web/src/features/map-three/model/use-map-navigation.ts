import { useState, useEffect, useMemo } from 'react';
import { useShallow } from 'zustand/react/shallow';
import { useMapStore } from './map-store';
import { useDeviceLocationStore } from '@/entities/device-location/model/device-location.store';

/**
 * 지도 내비게이션 및 위치 제어에 필요한 상태와 액션들을
 * 성능 최적화(Shallow) 처리하여 반환하는 커스텀 셀렉터 훅입니다.
 */
export function useMapNavigation() {
  // 1. 외부 스토어 구독
  const deviceLocation = useDeviceLocationStore((state) => state.location);
  const rawAccuracy = useDeviceLocationStore((state) => state.accuracy);
  const compassHeading = useDeviceLocationStore(
    (state) => state.compassHeading,
  );

  // 2. MapStore 상태 및 액션 추출
  const store = useMapStore(
    useShallow((state) => ({
      center: state.center,
      setCenter: state.setCenter,
      setCenterByLocation: state.setCenterByLocation,

      viewState: state.viewState,
      setViewState: state.setViewState,

      origin: state.origin,
      setOrigin: state.setOrigin,
      setOriginByLocation: state.setOriginByLocation,

      originOffset: state.originOffset,
      setOriginOffset: state.setOriginOffset,

      triggerDistanceReset: state.triggerDistanceReset,
      triggerOrientationReset: state.triggerOrientationReset,

      hoveredPos: state.hoveredPos,
      setHoveredPos: state.setHoveredPos,

      isFollowMode: state.isFollowMode,
      setIsFollowMode: state.setIsFollowMode,

      isInteracting: state.isInteracting,
      setIsInteracting: state.setIsInteracting,

      currentHour: state.currentHour,
      getThemeKey: state.getThemeKey,
    })),
  );
  const project = useMapStore((state) => state.project);

  // 3. 데이터 가공 (Accuracy 가드)
  // GPS 수신 불량으로 인한 비정상적인 값이나 null 처리
  const accuracy = useMemo(() => {
    if (rawAccuracy === null || rawAccuracy === undefined) return 10; // 기본값 10m
    return Math.min(Math.max(rawAccuracy, 4), 50); // 최소 4m(1칸), 최대 50m로 제한
  }, [rawAccuracy]);

  // 4. 좌표 계산 (절대 좌표)
  // 이렇게 하면 deviceLocation이나 center가 "유의미하게" 바뀔 때만 리렌더링을 유발합니다.
  const devicePos = useMemo(
    () => (deviceLocation ? project(deviceLocation) : null),
    [deviceLocation, project],
  );

  // 5. 로컬 좌표 계산 (Floating Origin 대응)
  const localDevicePos = useMemo(() => {
    if (!devicePos || !store.originOffset) return null;
    return {
      x: devicePos.x - store.originOffset.x,
      z: devicePos.z - store.originOffset.z,
    };
  }, [devicePos, store.originOffset]);

  const localHoveredPos = useMemo(() => {
    if (!store.hoveredPos || !store.originOffset) return null;
    return {
      x: store.hoveredPos.x - store.originOffset.x,
      z: store.hoveredPos.z - store.originOffset.z,
    };
  }, [store.hoveredPos, store.originOffset]);

  // 6. 이탈 판정 히스테리시스 로직
  const [isOffCenter, setIsOffCenter] = useState(false);

  /**
   * [Hysteresis Logic]
   * store 객체 전체를 넣지 않고, 계산에 필요한 구체적인 좌표값만 의존성에 넣습니다.
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
    // 🚀 store.center 전체가 아닌 x, z 값을 콕 집어서 감시
  }, [
    devicePos?.x,
    devicePos?.z,
    store.center?.x,
    store.center?.z,
    isOffCenter,
  ]);

  // 7. UI 편의 상태
  const canShowLocationControl = isOffCenter && !store.isFollowMode;
  const themeKey = useMemo(() => {
    return store.getThemeKey();
  }, [store.currentHour, store.getThemeKey]);

  return {
    ...store,
    devicePos,
    localDevicePos, // 🚀 1번 반영됨
    localHoveredPos,
    accuracy, // 🚀 3번 반영됨
    isOffCenter,
    deviceLocation,
    compassHeading,
    canShowLocationControl,
    themeKey,
  };
}
