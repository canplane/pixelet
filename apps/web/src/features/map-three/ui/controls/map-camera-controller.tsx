// 1. 외부 라이브러리 (External Libraries)
import { useEffect, useRef } from 'react';
import * as THREE from 'three';
import { useFrame, useThree } from '@react-three/fiber';
import { OrbitControls } from '@react-three/drei';
import { OrbitControls as OrbitControlsImpl } from 'three-stdlib';

// 2. 프로젝트 공통 타입 및 상수 (Shared / Features)
import { THREE_CONFIG } from '../../model/three-config';
import type { MeterPos } from '@/shared/types/map';

// 3. 기능 전용 상태 관리 (Feature Store)
import { useMapStore } from '../../model/map-store';
import { useMapNavigation } from '../../model/use-map-navigation';

const { DEFAULT_DISTANCE, DEFAULT_POLAR_ANGLE, MIN_DISTANCE, MAX_DISTANCE } =
  THREE_CONFIG.CAMERA;
const { FAR_CLIP, RESET_THRESHOLD } = THREE_CONFIG.RENDERING;
interface Props {
  worldGroupRef: React.RefObject<THREE.Group | null>;
}

export function MapCameraController({ worldGroupRef }: Props) {
  const { camera } = useThree();
  const controlsRef = useRef<OrbitControlsImpl>(null);

  /**
   * 🚀 [Floating Origin의 핵심: 누적 오프셋]
   * 지도의 '절대 원점'과 현재 Three.js '로컬 원점(0,0,0)' 사이의 거리 차이를 저장합니다.
   * 이 값이 있어야만 수천 킬로미터 떨어진 곳에서도 정밀한 좌표 계산이 가능합니다.
   */
  const originOffset = useMapStore((state) => state.originOffset);
  const setOriginOffset = useMapStore((state) => state.setOriginOffset);

  const isInitialized = useRef(false);
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
  const resetSignal = useMapStore((state) => state.cameraResetSignal);

  // 카메라의 가시거리를 설정값과 동기화
  useEffect(() => {
    camera.far = FAR_CLIP;
    camera.updateProjectionMatrix();
  }, [camera]);

  useEffect(() => {
    if (!devicePos) return;
    if (!origin) {
      setOrigin(devicePos);

      // 🚀 [추가] 앱이 처음 켜질 때, 초기 원점을 아예 누적 오프셋으로 설정합니다.
      // 이렇게 해야 카메라가 (1400만, 0, -440만)이 아닌 (0, 0, 0)에서 시작합니다.
      setOriginOffset({ x: devicePos.x, y: devicePos.y, z: devicePos.z });

      // 타겟 센터는 (0,0,0)으로 시작하게 됩니다 (내 위치가 곧 원점)
      const initialCenter = devicePos;
      setCenter(initialCenter);
      return;
    }

    /** * [절대 좌표 시스템 전환 알림]
     * 이제 origin과의 거리를 비교하여 원점을 리셋하던 로직은 필요 없습니다.
     * 모든 좌표가 Web Mercator 미터 단위로 직접 관리되기 때문입니다.
     * 만약 "내 위치 추적 모드"라면 아래 주석을 해제하세요.
     */
    // setCenter(project(deviceLocation));
  }, [devicePos, origin, setOrigin, setCenter]);

  const lastProcessedCenter = useRef<string>('');
  const lastUpdate = useRef({ distance: 0, bearing: 0 });
  const lastUpdateTime = useRef(0);

  // 애니메이션을 위한 목표값 저장용 Ref
  const flyState = useRef<{
    position: THREE.Vector3;
    lookAt: THREE.Vector3;
    active: boolean; // isAnimating
  }>({
    position: new THREE.Vector3(),
    lookAt: new THREE.Vector3(),
    active: false,
  });

  /**
   * [지형 및 거리 대응 동기화 함수]
   * 직선거리(distance)를 유지하며 타겟(지형) 위에 카메라를 배치합니다.
   */
  const syncCamera = (targetPos: MeterPos, distance: number) => {
    if (!controlsRef.current) return;
    const polar = controlsRef.current.getPolarAngle() || DEFAULT_POLAR_ANGLE;
    const azimuth = controlsRef.current.getAzimuthalAngle();

    // 구면 좌표계를 이용한 단순하고 명확한 계산
    const destX = targetPos.x + distance * Math.sin(polar) * Math.sin(azimuth);
    const destY = targetPos.y + distance * Math.cos(polar); // h = d * cos(θ)
    const destZ = targetPos.z + distance * Math.sin(polar) * Math.cos(azimuth);

    camera.position.set(destX, destY, destZ);
    controlsRef.current.target.set(targetPos.x, targetPos.y, targetPos.z);
    controlsRef.current.update();
  };

  /**
   * [애니메이션 보간 함수]
   * 순간이동 대신 목적지만 설정하고 애니메이션 플래그를 켭니다.
   */
  const flyTo = (targetPos: MeterPos, distance: number) => {
    if (!controlsRef.current) return;

    const polar = controlsRef.current.getPolarAngle() || DEFAULT_POLAR_ANGLE;
    const azimuth = controlsRef.current.getAzimuthalAngle();

    // 목적지 계산
    const destX = targetPos.x + distance * Math.sin(polar) * Math.sin(azimuth);
    const destY = targetPos.y + distance * Math.cos(polar);
    const destZ = targetPos.z + distance * Math.sin(polar) * Math.cos(azimuth);

    flyState.current.position.set(destX, destY, destZ);
    flyState.current.lookAt.set(targetPos.x, targetPos.y, targetPos.z);
    flyState.current.active = true;
  };

  /**
   * 📡 [절대 좌표 -> 로컬 좌표 변환 동기화]
   * Store에 저장된 '천만 단위 절대 좌표'를 현재 로컬 좌표계에 맞춰 변환하여 카메라를 이동시킵니다.
   */
  useEffect(() => {
    // 🚀 controlsRef.current 가 null인지 먼저 체크합니다. (Type Guard)
    if (!center || !origin || !controlsRef.current) return;

    // OrbitControls의 내부 active 상태 체크 (애니메이션 중 간섭 방지)
    if (!center || !origin || (controlsRef.current as any).active) return;

    // 절대 좌표(center)에서 누적 오프셋(originOffset)을 빼서 현재 Scene에서의 위치를 구합니다.
    const localTarget: MeterPos = {
      x: center.x - originOffset.x,
      y: center.y - originOffset.y,
      z: center.z - originOffset.z,
    };

    const currentDist = camera.position.distanceTo(controlsRef.current.target);
    const finalDistance = currentDist > 1 ? currentDist : DEFAULT_DISTANCE;

    if (!isInitialized.current) {
      syncCamera(localTarget, finalDistance);
      isInitialized.current = true;
    } else {
      flyTo(localTarget, finalDistance);
    }
  }, [center, origin, originOffset]); // 🚀 originOffset이 스토어 값이므로 의존성에 추가

  // 거리 리셋 감지
  useEffect(() => {
    if (!controlsRef.current || resetSignal.distance === 0) return;
    flyTo(controlsRef.current.target, DEFAULT_DISTANCE); // 현재 각도는 유지한 채 거리만 리셋
  }, [resetSignal.distance]);

  // 방향 리셋 감지
  useEffect(() => {
    if (!controlsRef.current || resetSignal.orientation === 0) return;

    // 1. 즉시 변경(setPolarAngle)은 삭제하세요. (애니메이션과 충돌함)

    const target = controlsRef.current.target;
    const currentDist = camera.position.distanceTo(target);

    // 2. 목표 위치(Dest)만 정확히 계산해서 flyState에 넘깁니다.
    const polar = DEFAULT_POLAR_ANGLE;
    const azimuth = 0; // 정북

    const destX = target.x + currentDist * Math.sin(polar) * Math.sin(azimuth);
    const destY = target.y + currentDist * Math.cos(polar);
    const destZ = target.z + currentDist * Math.sin(polar) * Math.cos(azimuth);

    flyState.current.position.set(destX, destY, destZ);
    flyState.current.lookAt.set(target.x, target.y, target.z);
    flyState.current.active = true; // 이제 useFrame이 부드럽게 옮겨줄 겁니다.
  }, [resetSignal.orientation]);

  useFrame((state) => {
    if (!controlsRef.current || !worldGroupRef.current) return;

    /**
     * 🔄 [Floating Origin 실시간 감시]
     * 카메라가 원점(0,0,0)에서 2,000m 이상 멀어지면 정밀도 저하(지터링, 흑백 현상)가 시작됩니다.
     * 이때 세상을 반대 방향으로 밀어 카메라를 다시 원점으로 소환합니다.
     */
    if (camera.position.length() > RESET_THRESHOLD) {
      // 1. 현재 카메라가 보고 있는 지점(Target)을 새로운 기준점으로 잡습니다.
      const offset = controlsRef.current.target.clone();

      // 2. 누적 오프셋에 더해줍니다. (이동한 만큼 기록)
      // 🚀 [수정] 내부 Ref 대신 Store 업데이트
      const newTotalOffset = {
        x: originOffset.x + offset.x,
        y: originOffset.y + offset.y,
        z: originOffset.z + offset.z,
      };
      setOriginOffset(newTotalOffset);

      // 3. [핵심] 세계 전체를 반대 방향으로 밀어버립니다.
      // 자식 객체(OSM, 마커 등)들은 worldGroup 안에 있으므로 함께 이동합니다.
      worldGroupRef.current.position.sub(offset);

      // 4. 카메라와 컨트롤러 타겟도 동일한 만큼 이동시켜 상대적 시점은 유지합니다.
      camera.position.sub(offset);
      controlsRef.current.target.sub(offset);

      // 5. [플루언트 디자인 정밀도 유지] 즉시 행렬 업데이트
      // 한 프레임의 오차도 없이 위치를 동기화하여 깜빡임을 방지합니다.
      worldGroupRef.current.updateMatrixWorld(true);
      camera.updateMatrixWorld(true);

      // 6. 비행 애니메이션 중이었다면 목표 좌표도 보정합니다.
      if (flyState.current.active) {
        flyState.current.position.sub(offset);
        flyState.current.lookAt.sub(offset);
      }

      controlsRef.current.update();
      console.log(
        '🌐 Floating Origin Reset: Precision maintained at',
        originOffset,
      );
    }

    // --- 부드러운 비행 처리 (Fly Animation) ---
    if (flyState.current.active) {
      const { position: targetPos, lookAt: targetLookAt } = flyState.current;

      // 🚀 기존 0.1에서 0.05로 변경 (숫자가 작을수록 더 느리고 부드러움)
      const LERP_SPEED = 0.05;

      // 1. 카메라 위치 보간
      camera.position.lerp(targetPos, LERP_SPEED);

      // 2. 컨트롤러 타겟(중심점) 보간
      controlsRef.current.target.lerp(targetLookAt, LERP_SPEED);

      controlsRef.current.update();

      // 도착 판정 로직 (속도가 느려졌으므로 판정 거리를 조금 더 여유 있게 잡을 수도 있습니다)
      if (camera.position.distanceTo(targetPos) < 0.1) {
        flyState.current.active = false;
      }
    }

    // --- 상태 업데이트 보고 ---
    const now = state.clock.getElapsedTime();
    if (now - lastUpdateTime.current < 0.05) return;

    const distance = camera.position.distanceTo(controlsRef.current.target); // 직선 거리 (줌 레벨)
    const altitude = camera.position.y - controlsRef.current.target.y; // 실제 수직 고도 (Altitude)

    const azimuth = controlsRef.current.getAzimuthalAngle();
    let bearing = azimuth * (180 / Math.PI);

    // [중요] rotateSpeed={-1}일 때 나침반과 동기화
    bearing = -bearing;

    if (bearing <= -180) bearing += 360;
    if (bearing > 180) bearing -= 360;
    if (Math.abs(bearing) < 0.1) bearing = 0;

    const polarAngle = controlsRef.current.getPolarAngle();
    // const pitch = (polarAngle * 180) / Math.PI;

    const distanceDiff = Math.abs(lastUpdate.current.distance - distance);
    const bearingDiff = Math.abs(lastUpdate.current.bearing - bearing);

    if (distanceDiff > distance * 0.005 || bearingDiff > 0.1) {
      lastUpdate.current = { distance, bearing };
      lastUpdateTime.current = now;
      setViewState({ distance, altitude, bearing, polarAngle });
    }
  });

  return (
    <OrbitControls
      ref={controlsRef}
      onStart={() => {
        setIsFollowMode(false);
        flyState.current.active = false;
        setIsInteracting(true); // 👈 UI를 투명하게 만들고 격리시킴
      }}
      makeDefault
      mouseButtons={{
        LEFT: THREE.MOUSE.PAN, // 왼쪽 클릭: 드래그 이동
        MIDDLE: THREE.MOUSE.DOLLY, // 휠 클릭: 확대/축소
        RIGHT: THREE.MOUSE.ROTATE, // 오른쪽 클릭: 회전
      }}
      touches={{
        ONE: THREE.TOUCH.PAN, // 한 손가락: 이동
        TWO: THREE.TOUCH.DOLLY_PAN, // 두 손가락: 핀치 줌 + 회전
      }}
      // 회전 관련 세밀한 설정
      enableRotate={true} // 회전 활성화
      rotateSpeed={1} // 🚀 수평 회전을 위해 -1 유지
      // 댐핑(관성) 설정
      enableDamping={true}
      dampingFactor={0.08} // 관성을 적절히 주어 부드럽게 멈춤
      // 기타 거리 제한
      minDistance={MIN_DISTANCE}
      maxDistance={MAX_DISTANCE} // 너무 크면 정밀도가 떨어질 수 있음
      // 이동 및 회전 제한
      screenSpacePanning={false} // 드래그 시 카메라가 화면 방향이 아닌 '지면(바닥)'을 따라 움직이게 합니다.
      maxPolarAngle={Math.PI / 2.1} // 수직 회전의 아래쪽 한계 (90도 직전)
      minPolarAngle={0} // 수직 회전의 위쪽 한계 (0도 근처)
      onEnd={() => {
        setIsInteracting(false);
        if (controlsRef.current) {
          const t = controlsRef.current.target;

          /**
           * 📍 [로컬 좌표 -> 절대 좌표 변환 보고]
           * 로컬의 작은 좌표(t)에 누적된 오프셋을 더해 '진짜 지구상의 좌표'를 추출합니다.
           */
          const absolutePos = {
            x: t.x + originOffset.x,
            y: t.y + originOffset.y,
            z: t.z + originOffset.z,
          };

          lastProcessedCenter.current = JSON.stringify(absolutePos);
          setCenter(absolutePos); // 상용 서비스 수준의 절대 좌표 저장
        }
      }}
    />
  );
}
