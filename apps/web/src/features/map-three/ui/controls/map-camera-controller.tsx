import { useEffect, useRef } from 'react';
import * as THREE from 'three';
import { useFrame, useThree } from '@react-three/fiber';
import { OrbitControls } from '@react-three/drei';
import { OrbitControls as OrbitControlsImpl } from 'three-stdlib';

import { VOXEL_CONFIG } from '@pixelet/shared/entities/voxel/voxel-config';
import { THREE_CONFIG } from '@/entities/osm/config/three-config';
import type { MeterPos } from '@pixelet/shared/shared/model/coords';
import { useMapStore } from '@/entities/osm/model/map-store';
import { useMapNavigation } from '@/entities/osm/model/use-map-navigation';
import { OSM_CONSTANTS } from '@pixelet/shared/entities/osm/model/osm-constants';

// 🎯 [utility-import] 좌표 변환 유틸리티 적용
import { fromThreePos, toThreePos } from '@/shared/lib/three/three-coords';
import { useCoordinateConverter } from '../../../../entities/osm/lib/use-coordinate-system';

const { TILE_SIZE, TILE_BUFFER } = OSM_CONSTANTS;

const { DEFAULT_DISTANCE, DEFAULT_POLAR_ANGLE, MIN_DISTANCE, MAX_DISTANCE } =
  THREE_CONFIG.CAMERA;
const { FAR_CLIP } = THREE_CONFIG.RENDERING;

interface MapCameraControllerProps {
  worldGroupRef: React.RefObject<THREE.Group | null>;
}

/**
 * [map-camera-controller]
 * Floating Origin 시스템의 핵심 컨트롤러입니다.
 *
 * 1. [physics]: OrbitControls를 통한 카메라 물리 제어
 * 2. [floating-origin]: 타일 기반 양자화(quantization) 리셋을 통해 부동소수점 정밀도 유지
 * 3. [sync]: 로컬(Three.js) 좌표와 절대(Geographic) 좌표 간의 실시간 동기화
 * 4. [bridge-frame]: Origin 변경 시 발생하는 1프레임 깜빡임(Flicker)을 수동 조작으로 보정
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
    devicePos,
    setIsFollowMode,
    setIsInteracting,
  } = useMapNavigation();

  // 🎯 [converter-hooks] 양방향 변환 함수 가져오기
  const { localToGlobal, globalToLocal } = useCoordinateConverter();

  const resetSignal = useMapStore((state) => state.cameraResetSignal);

  // [config-sync]
  useEffect((): void => {
    camera.far = FAR_CLIP;
    camera.updateProjectionMatrix();
  }, [camera]);

  // [origin-initialization]
  useEffect((): void => {
    if (!devicePos || origin.x !== 0) return;

    const snappedInitial: MeterPos = VOXEL_CONFIG.snapToVoxel(devicePos);
    setOrigin(snappedInitial);
    setCenter(devicePos);
  }, [devicePos, origin]);

  /**
   * 🎯 [Bridge-Frame-Reset]
   * useFrame에서 수동으로 밀어버린 worldGroup의 위치를,
   * 리액트가 리렌더링(새로운 origin 반영)을 완료한 시점에 다시 0으로 초기화합니다.
   * 이로써 [수동 이동 -> 리액트 업데이트 -> 초기화] 사이클이 완성되어 깜빡임이 사라집니다.
   */
  useEffect(() => {
    if (worldGroupRef.current) {
      worldGroupRef.current.position.set(0, 0, 0);
      worldGroupRef.current.updateMatrixWorld(true);
    }
  }, [origin]); // origin 변경 감지

  const lastProcessedCenter = useRef<string>('');
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

  // [camera-sync-logic]
  const syncCamera = (targetPos: MeterPos, distance: number): void => {
    if (!controlsRef.current) return;
    const polar: number =
      controlsRef.current.getPolarAngle() || DEFAULT_POLAR_ANGLE;
    const azimuth: number = controlsRef.current.getAzimuthalAngle();

    // Spherical -> Cartesian
    const destX = targetPos.x + distance * Math.sin(polar) * Math.sin(azimuth);
    const destY = targetPos.y + distance * Math.cos(polar);
    const destZ = targetPos.z + distance * Math.sin(polar) * Math.cos(azimuth);

    camera.position.set(destX, destY, destZ);
    controlsRef.current.target.set(targetPos.x, targetPos.y, targetPos.z);
    controlsRef.current.update();
  };

  // [fly-interpolation]
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
   * [coordinate-synchronization]
   * 절대 좌표(global-center)가 변경될 때만 실행됩니다.
   * * 🚫 [Fix-SnapBack]: 의존성에서 'origin'을 제거했습니다.
   * Floating Origin이 바뀌는 것은 내부적인 좌표 이동일 뿐, 카메라가 'Center'로 되돌아가야 함을 의미하지 않습니다.
   * 이미 useFrame에서 수동으로 시각적 위치를 보정했으므로, 여기서 또 움직이면 안 됩니다.
   */
  useEffect((): void => {
    if (!center || !controlsRef.current) return;

    // 사용자가 상호작용 중일 때는 강제 이동 금지
    if ((controlsRef.current as any).active) return;

    // 🎯 [Loop-Prevention] 방금 onEnd에서 내가 설정한 center라면, 카메라를 또 움직일 필요가 없음
    // (이게 없으면 드래그 끝난 직후 아주 미세하게 카메라가 튀는 현상이 발생할 수 있음)
    if (lastProcessedCenter.current === JSON.stringify(center)) return;

    const localGeoPos = globalToLocal(center);

    // 🚀 [toThreePos] 유틸리티 사용하여 변환 (Vector3Tuple 분해)
    const threePos = toThreePos(localGeoPos);
    const localTargetVec = new THREE.Vector3(...threePos);

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
  }, [center, globalToLocal]); // 🎯 origin 제거, globalToLocal 추가

  // [signal-handlers]
  useEffect((): void => {
    if (!controlsRef.current || resetSignal.distance === 0) return;
    flyTo(controlsRef.current.target, DEFAULT_DISTANCE);
  }, [resetSignal.distance]);

  useEffect((): void => {
    if (!controlsRef.current || resetSignal.orientation === 0) return;
    const target: THREE.Vector3 = controlsRef.current.target;
    const currentDist: number = camera.position.distanceTo(target);
    const polar: number = DEFAULT_POLAR_ANGLE;
    const destX: number = target.x;
    const destY: number = target.y + currentDist * Math.cos(polar);
    const destZ: number = target.z + currentDist * Math.sin(polar);
    flyState.current.position.set(destX, destY, destZ);
    flyState.current.lookAt.set(target.x, target.y, target.z);
    flyState.current.active = true;
  }, [resetSignal.orientation, camera.position]);

  // [main-frame-loop]
  useFrame((state) => {
    if (!controlsRef.current || !worldGroupRef.current) return;

    // --- 1. Floating Origin Logic ---
    const t = controlsRef.current.target;

    /**
     * 🎯 [Standardized-Unswizzling]
     * 기존의 하드코딩된 좌표 복원 로직(-z -> y)을 표준 유틸리티(fromThreePos)로 대체합니다.
     * Three.js 좌표(Local) -> 지리 좌표(Local Geo)
     */
    const localGeo = fromThreePos([t.x, t.y, t.z]);

    const absX = localGeo.x + origin.x;
    const absY = localGeo.y + origin.y;

    const currentTx = Math.floor(absX / TILE_SIZE);
    const currentTy = Math.floor(absY / TILE_SIZE);
    const originTx = Math.floor(origin.x / TILE_SIZE);
    const originTy = Math.floor(origin.y / TILE_SIZE);

    const diffX = Math.abs(currentTx - originTx);
    const diffY = Math.abs(currentTy - originTy);

    if (
      !flyState.current.active &&
      (diffX >= TILE_BUFFER || diffY >= TILE_BUFFER)
    ) {
      const newOriginX = currentTx * TILE_SIZE + TILE_SIZE / 2;
      const newOriginY = currentTy * TILE_SIZE + TILE_SIZE / 2;
      const moveX = newOriginX - origin.x;
      const moveY = newOriginY - origin.y;

      const newOrigin: MeterPos = {
        x: newOriginX,
        y: newOriginY,
        z: origin.z,
      };
      setOrigin(newOrigin); // React Update Trigger

      // Floating Origin Shift Vector
      // 🚀 [toThreePos] 이동 벡터도 표준 변환을 사용하여 생성 (Geo [dx, dy] -> Three [dx, 0, -dy])
      const moveVecTuple = toThreePos({ x: moveX, y: moveY });
      const moveVec = new THREE.Vector3(...moveVecTuple);

      /**
       * 🎯 [Sync-Correction]
       * 1. 카메라와 컨트롤 타겟을 이동 (Local 좌표계 유지)
       * 2. WorldGroup도 동일하게 이동 (sub)하여,
       * 타일들이 아직 '오래된 좌표'를 가지고 있는 1프레임 동안 위치를 보정함.
       * * (원리: Origin이 +100 이동 -> 카메라 -100 이동 -> 타일도 -100 이동해야 시각적 고정)
       */
      camera.position.sub(moveVec);
      controlsRef.current.target.sub(moveVec);
      worldGroupRef.current.position.sub(moveVec);

      camera.updateMatrixWorld(true);
      controlsRef.current.update();
      // worldGroupRef 업데이트는 필수는 아니지만 안전을 위해 수행
      worldGroupRef.current.updateMatrixWorld(true);

      console.log(
        `[FloatingOrigin] Shifted to Tile(${currentTx}, ${currentTy})`,
      );
    }

    // --- 2. Fly Animation ---
    if (flyState.current.active) {
      const { position: targetPos, lookAt: targetLookAt } = flyState.current;
      camera.position.lerp(targetPos, 0.05);
      controlsRef.current.target.lerp(targetLookAt, 0.05);
      controlsRef.current.update();
      if (camera.position.distanceTo(targetPos) < 0.5) {
        flyState.current.active = false;
      }
    }

    // --- 3. View State Reporting ---
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
        LEFT: THREE.MOUSE.PAN,
        MIDDLE: THREE.MOUSE.DOLLY,
        RIGHT: THREE.MOUSE.ROTATE,
      }}
      touches={{
        ONE: THREE.TOUCH.PAN,
        TWO: THREE.TOUCH.DOLLY_ROTATE,
      }}
      enableRotate={true}
      rotateSpeed={1}
      enableDamping={true}
      dampingFactor={0.08}
      minDistance={MIN_DISTANCE}
      maxDistance={MAX_DISTANCE}
      screenSpacePanning={false}
      maxPolarAngle={Math.PI / 2.1}
      minPolarAngle={0}
      onEnd={(): void => {
        setIsInteracting(false);
        if (controlsRef.current) {
          const t: THREE.Vector3 = controlsRef.current.target;

          // [Safety Check]
          if (
            !Number.isFinite(t.x) ||
            !Number.isFinite(t.y) ||
            !Number.isFinite(t.z)
          ) {
            console.warn('[Camera] Invalid target detected, skipping update');
            return;
          }

          // 🚀 [fromThreePos] 표준 유틸리티 사용 (Vector3Tuple 타입 단언)
          const localMeterPos = fromThreePos([t.x, t.y, t.z]);
          const absolutePos = localToGlobal(localMeterPos);

          lastProcessedCenter.current = JSON.stringify(absolutePos);
          setCenter(absolutePos);
        }
      }}
    />
  );
}
