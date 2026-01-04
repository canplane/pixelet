import { useRef, useLayoutEffect, type RefObject } from 'react';
import { useFrame, useThree } from '@react-three/fiber';
import { OrbitControls as OrbitControlsImpl } from 'three-stdlib';
import { useMapNavigation } from '@/entities/geo/model/use-map-navigation';
import type { CameraFlyTarget } from './use-camera-movement';

interface UseCameraFrameProps {
  controlsRef: React.RefObject<OrbitControlsImpl | null>;
  flyTarget: RefObject<CameraFlyTarget>;
}

export function useCameraFrame({
  controlsRef,
  flyTarget,
}: UseCameraFrameProps) {
  const { cameraControlMode, setViewState, isFlying, setIsFlying } =
    useMapNavigation();
  const { camera } = useThree();

  const lastUpdate = useRef({ distance: 0, bearing: 0 });
  const lastUpdateTime = useRef(0);

  // 1. 📊 [Reporting] 물리 엔진의 상태를 스토어에 동기화 (Event-driven)
  useLayoutEffect(() => {
    const controls = controlsRef.current;
    if (!controls) return;

    const handleReport = () => {
      // 애니메이션 중일 때는 useFrame이 update를 호출하므로 여기서 중복 보고를 막기 위해 throttle 적용 가능
      const now = performance.now();
      if (now - lastUpdateTime.current < 50) return; // 50ms (20fps 수준)로 UI 보고 제한

      const distance = camera.position.distanceTo(controls.target);
      const altitude = camera.position.y - controls.target.y;
      const polarAngle = controls.getPolarAngle();
      let bearing = -controls.getAzimuthalAngle() * (180 / Math.PI);

      // Bearing 정규화 (-180 ~ 180)
      if (bearing <= -180) bearing += 360;
      if (bearing > 180) bearing -= 360;

      // 🎯 [Threshold Guard] 유의미한 변화일 때만 스토어 전파
      const isChanged =
        Math.abs(lastUpdate.current.distance - distance) > distance * 0.005 ||
        Math.abs(lastUpdate.current.bearing - bearing) > 0.1;

      if (isChanged) {
        lastUpdate.current = { distance, bearing };
        lastUpdateTime.current = now;
        setViewState({ distance, altitude, bearing, polarAngle });
      }
    };

    controls.addEventListener('change', handleReport);
    return () => controls.removeEventListener('change', handleReport);
  }, [controlsRef, camera, setViewState]);

  // 2. 🚀 [Animation] flyState 계획 집행 (Exponential Interpolation)
  useFrame((_, delta) => {
    const controls = controlsRef.current;
    if (!controls || !isFlying) return;

    // 🔒 [Interrupt] 사용자 개입 시 비행 즉시 취소
    if (cameraControlMode === 'user') {
      setIsFlying(false);
      return;
    }

    const { dstPos, dstLookAt } = flyTarget.current;

    // 지수적 보간: delta를 활용해 프레임 독립적인 smoothing factor 적용
    const alpha = 1 - Math.pow(0.001, delta);

    camera.position.lerp(dstPos, alpha);
    controls.target.lerp(dstLookAt, alpha);

    // controls.update()가 내부적으로 'change' 이벤트를 발생시켜 위 Reporting 로직을 실행함
    controls.update();

    // 🏁 [Termination] 수렴 조건 확인
    if (camera.position.distanceTo(dstPos) < 0.05) {
      camera.position.copy(dstPos);
      controls.target.copy(dstLookAt);
      setIsFlying(false);
    }
  });
}
