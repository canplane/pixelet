import { useEffect, useRef } from 'react';
import * as THREE from 'three';
import { useFrame, useThree } from '@react-three/fiber';
import { OrbitControls } from '@react-three/drei';
import { OrbitControls as OrbitControlsImpl } from 'three-stdlib';

import { VOXEL_CONFIG } from '@pixelet/shared/entities/voxel/voxel-config';
import { THREE_CONFIG } from '../../model/three-config';
import type { MeterPos } from '@pixelet/shared/shared/model/coords';
import { useMapStore } from '../../model/map-store';
import { useMapNavigation } from '../../model/use-map-navigation';

const { DEFAULT_DISTANCE, DEFAULT_POLAR_ANGLE, MIN_DISTANCE, MAX_DISTANCE } =
  THREE_CONFIG.CAMERA;
const { FAR_CLIP, RESET_THRESHOLD } = THREE_CONFIG.RENDERING;

interface MapCameraControllerProps {
  worldGroupRef: React.RefObject<THREE.Group | null>;
}

/**
 * [map-camera-controller]
 * 지도의 카메라 물리 엔진과 좌표계 동기화를 관리합니다.
 * Floating Origin 기술을 통해 거대 좌표계에서의 정밀도 문제를 해결합니다.
 */
export function MapCameraController({
  worldGroupRef,
}: MapCameraControllerProps) {
  const { camera } = useThree();
  const controlsRef = useRef<OrbitControlsImpl>(null);
  const isInitialized = useRef<boolean>(false);

  const {
    center,
    setCenter,
    setViewState,
    origin,
    setOrigin,
    originOffset,
    setOriginOffset,
    devicePos,
    setIsFollowMode,
    setIsInteracting,
  } = useMapNavigation();

  const resetSignal = useMapStore((state) => state.cameraResetSignal);

  // 카메라의 가시거리를 설정값과 동기화
  useEffect((): void => {
    camera.far = FAR_CLIP;
    camera.updateProjectionMatrix();
  }, [camera]);

  /**
   * [initialize-origin]
   * 최초 기기 위치 수신 시 월드의 원점을 격자 눈금(4m)에 맞춰 초기화합니다.
   * [initial-snap-apply] 최초 원점 설정 시에도 VOXEL_CONFIG.snapToVoxel를 사용하여
   * 월드의 시작점을 4m 격자 눈금에 정확히 맞춥니다.
   */
  useEffect((): void => {
    if (!devicePos || origin) return;

    setOrigin(devicePos);
    const snappedInitial: MeterPos = VOXEL_CONFIG.snapToVoxel(devicePos);
    setOriginOffset(snappedInitial);
    setCenter(devicePos); // 센터는 부드러운 시작을 위해 소수점 유지
  }, [devicePos, origin]);

  const lastProcessedCenter = useRef<string>('');

  /**
   * [performance-optimization-refs]
   * 불필요한 상태 업데이트를 방지하기 위해 마지막 보고 값을 추적합니다.
   */
  const lastUpdate = useRef<{ distance: number; bearing: number }>({
    distance: 0,
    bearing: 0,
  });
  const lastUpdateTime = useRef<number>(0);

  const flyState = useRef<{
    position: THREE.Vector3;
    lookAt: THREE.Vector3;
    active: boolean;
  }>({
    position: new THREE.Vector3(),
    lookAt: new THREE.Vector3(),
    active: false,
  });

  /**
   * [sync-camera-immediate]
   * 직선거리(distance)를 유지하며 타겟(지형) 위에 카메라를 즉시 동기화합니다.
   */
  const syncCamera = (targetPos: MeterPos, distance: number): void => {
    if (!controlsRef.current) return;
    const polar: number =
      controlsRef.current.getPolarAngle() || DEFAULT_POLAR_ANGLE;
    const azimuth: number = controlsRef.current.getAzimuthalAngle();

    // Three.js 좌표계 기준 (Y-Up)
    const destX = targetPos.x + distance * Math.sin(polar) * Math.sin(azimuth);
    const destY = targetPos.y + distance * Math.cos(polar); // Three Y (Altitude)
    const destZ = targetPos.z + distance * Math.sin(polar) * Math.cos(azimuth);

    camera.position.set(destX, destY, destZ);
    controlsRef.current.target.set(targetPos.x, targetPos.y, targetPos.z);
    controlsRef.current.update();
  };

  /**
   * [fly-to-interpolation]
   * 목표 좌표로 부드럽게 이동하기 위한 애니메이션 상태를 설정합니다.
   */
  const flyTo = (targetPos: MeterPos, distance: number): void => {
    if (!controlsRef.current) return;

    const polar: number =
      controlsRef.current.getPolarAngle() || DEFAULT_POLAR_ANGLE;
    const azimuth: number = controlsRef.current.getAzimuthalAngle();

    const destX: number =
      targetPos.x + distance * Math.sin(polar) * Math.sin(azimuth);
    const destY: number = targetPos.y + distance * Math.cos(polar);
    const destZ: number =
      targetPos.z + distance * Math.sin(polar) * Math.cos(azimuth);

    flyState.current.position.set(destX, destY, destZ);
    flyState.current.lookAt.set(targetPos.x, targetPos.y, targetPos.z);
    flyState.current.active = true;
  };

  /**
   * [coordinate-sync-effect]
   * 절대 좌표(center)에서 스냅된 오프셋(originOffset)을 빼서
   * Three.js 공간의 '로컬 좌표'를 산출하고 카메라를 동기화합니다.
   * 💡 중요: Three.js의 Z는 지리적 Y(남북)에 대응되며 부호가 반전됩니다.
   */
  useEffect((): void => {
    if (!center || !origin || !controlsRef.current) return;
    if ((controlsRef.current as any).active) return;

    // useEffect 내부 예시
    const localTargetVec = new THREE.Vector3(
      center.x - originOffset.x,
      center.z - originOffset.z, // 고도
      -(center.y - originOffset.y), // 남북
    );

    const currentDist: number = camera.position.distanceTo(
      controlsRef.current.target,
    );
    const finalDistance: number =
      currentDist > 1 ? currentDist : DEFAULT_DISTANCE;

    if (!isInitialized.current) {
      syncCamera(localTargetVec, finalDistance);
      isInitialized.current = true;
    } else {
      flyTo(localTargetVec, finalDistance);
    }
  }, [center, origin, originOffset]);

  // [reset-logic-distance] 거리 리셋 신호 처리
  useEffect((): void => {
    if (!controlsRef.current || resetSignal.distance === 0) return;
    flyTo(controlsRef.current.target, DEFAULT_DISTANCE);
  }, [resetSignal.distance]);

  // [reset-logic-orientation] 방향(정북) 리셋 신호 처리
  useEffect((): void => {
    if (!controlsRef.current || resetSignal.orientation === 0) return;

    const target: THREE.Vector3 = controlsRef.current.target;
    const currentDist: number = camera.position.distanceTo(target);
    const polar: number = DEFAULT_POLAR_ANGLE;
    const azimuth: number = 0; // 정북 방향

    const destX: number =
      target.x + currentDist * Math.sin(polar) * Math.sin(azimuth);
    const destY: number = target.y + currentDist * Math.cos(polar);
    const destZ: number =
      target.z + currentDist * Math.sin(polar) * Math.cos(azimuth);

    flyState.current.position.set(destX, destY, destZ);
    flyState.current.lookAt.set(target.x, target.y, target.z);
    flyState.current.active = true;
  }, [resetSignal.orientation, camera.position]);

  /**
   * [update-frame-loop]
   * 매 프레임마다 Floating Origin 리셋 판정 및 애니메이션 보간을 처리합니다.
   */
  useFrame((state): void => {
    if (!controlsRef.current || !worldGroupRef.current) return;

    /**
     * [floating-origin-reset]
     * 🔄 [origin-reset-snap]
     * 카메라가 멀어지면 세상을 다시 (0,0,0) 근처로 당깁니다.
     * 이때 '스냅된 거리'만큼 이동해야 배경 격자가 튀지 않습니다.
     */
    if (
      !flyState.current.active &&
      camera.position.length() > RESET_THRESHOLD
    ) {
      const rawOffset: THREE.Vector3 = controlsRef.current.target.clone();

      // Three.js 로컬 좌표를 다시 지리적 좌표 규격으로 역변환하여 스냅
      const snappedStep: MeterPos = VOXEL_CONFIG.snapToVoxel({
        x: rawOffset.x,
        y: -rawOffset.z, // Three -Z -> Geo Y
        z: rawOffset.y, // Three Y -> Geo Z
      });

      const newTotalOffset: MeterPos = {
        x: originOffset.x + snappedStep.x,
        y: originOffset.y + snappedStep.y,
        z: originOffset.z + snappedStep.z,
      };
      setOriginOffset(newTotalOffset);

      // 세계 전체와 카메라를 정확히 '격자 단위'로 반대 방향 이동
      // 월드 그룹 및 카메라 이동 (Three.js 공간 기준이므로 raw x, y, z 사용)
      const moveVec = new THREE.Vector3(
        snappedStep.x,
        snappedStep.z, // Geo Z -> Three Y
        -snappedStep.y, // Geo Y -> Three -Z
      );

      worldGroupRef.current.position.sub(moveVec);
      camera.position.sub(moveVec);
      controlsRef.current.target.sub(moveVec);

      worldGroupRef.current.updateMatrixWorld(true);
      camera.updateMatrixWorld(true);
      controlsRef.current.update();
    }

    /**
     * [fly-animation-processing]
     * 설정된 목적지까지 부드러운 Lerp 보간을 적용합니다.
     */
    if (flyState.current.active) {
      const { position: targetPos, lookAt: targetLookAt } = flyState.current;
      camera.position.lerp(targetPos, 0.05);
      controlsRef.current.target.lerp(targetLookAt, 0.05);
      controlsRef.current.update();

      if (camera.position.distanceTo(targetPos) < 0.5) {
        flyState.current.active = false;
      }
    }

    /**
     * [view-state-reporting]
     * 카메라의 거리, 고도, 방위각 등을 계산하여 글로벌 상태에 보고합니다.
     * [last-update-throttle] 매 프레임 업데이트 방지를 위해 유의미한 변화가 있을 때만 보고합니다.
     */
    const now: number = state.clock.getElapsedTime();
    if (now - lastUpdateTime.current < 0.05) return;

    const distance: number = camera.position.distanceTo(
      controlsRef.current.target,
    );
    const altitude: number = camera.position.y - controlsRef.current.target.y;
    const azimuth: number = controlsRef.current.getAzimuthalAngle();
    let bearing: number = -azimuth * (180 / Math.PI);

    if (bearing <= -180) bearing += 360;
    if (bearing > 180) bearing -= 360;
    if (Math.abs(bearing) < 0.1) bearing = 0;

    const polarAngle: number = controlsRef.current.getPolarAngle();

    // 유의미한 변화량 체크 (최적화)
    const distanceDiff: number = Math.abs(
      lastUpdate.current.distance - distance,
    );
    const bearingDiff: number = Math.abs(lastUpdate.current.bearing - bearing);

    if (distanceDiff > distance * 0.005 || bearingDiff > 0.1) {
      lastUpdate.current = { distance, bearing };
      lastUpdateTime.current = now;
      setViewState({ distance, altitude, bearing, polarAngle });
    }
  });

  return (
    <OrbitControls
      ref={controlsRef}
      onStart={(): void => {
        setIsFollowMode(false);
        flyState.current.active = false;
        setIsInteracting(true);
      }}
      makeDefault
      mouseButtons={{
        LEFT: THREE.MOUSE.PAN, // 왼쪽 클릭: 드래그 이동
        MIDDLE: THREE.MOUSE.DOLLY, // 휠 클릭: 확대/축소
        RIGHT: THREE.MOUSE.ROTATE, // 오른쪽 클릭: 회전
      }}
      touches={{
        ONE: THREE.TOUCH.PAN, // 한 손가락: 이동
        TWO: THREE.TOUCH.DOLLY_ROTATE, // 두 손가락: 핀치 줌 + 회전
      }}
      enableRotate={true}
      rotateSpeed={1}
      enableDamping={true} // 관성을 적절히 주어 부드럽게 멈춤
      dampingFactor={0.08}
      minDistance={MIN_DISTANCE}
      maxDistance={MAX_DISTANCE}
      screenSpacePanning={false} // 드래그 시 카메라가 바닥면을 따라 움직이도록 제한
      maxPolarAngle={Math.PI / 2.1} // [limit-down] 지형 아래를 보지 못하도록 제한
      minPolarAngle={0} // [limit-up] 정수리 위를 넘어가 뒤집히지 않게 제한
      onEnd={(): void => {
        setIsInteracting(false);
        if (controlsRef.current) {
          const t: THREE.Vector3 = controlsRef.current.target;

          /**
           * 📍 [absolute-position-reverse]
           * Three.js 로컬 축을 다시 지리적 표준(X, Y, Z)으로 변환
           */
          const absolutePos: MeterPos = {
            x: t.x + originOffset.x,
            y: -t.z + originOffset.y, // Three -Z -> Geo Y (North)
            z: t.y + originOffset.z, // Three Y -> Geo Z (Alt)
          };

          lastProcessedCenter.current = JSON.stringify(absolutePos);
          setCenter(absolutePos);
        }
      }}
    />
  );
}
