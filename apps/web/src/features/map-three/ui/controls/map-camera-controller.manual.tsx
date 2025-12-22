// 1. 외부 라이브러리 (External Libraries)
import { useEffect, useRef } from 'react';
import * as THREE from 'three';
import { useFrame, useThree } from '@react-three/fiber';
import { OrbitControls } from '@react-three/drei';
import { OrbitControls as OrbitControlsImpl } from 'three-stdlib';

// 2. 프로젝트 공통 타입 및 상수 (Shared / Features)
import { projection } from '@/shared/lib/geo/projection';
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
  const { camera, gl, scene } = useThree();
  const controlsRef = useRef<OrbitControlsImpl>(null);

  const isInitialized = useRef(false);

  const {
    center,
    setCenter,
    setViewState,
    origin,
    setOrigin,

    /**
     * 🚀 [Floating Origin의 핵심: 누적 오프셋]
     * 지도의 '절대 원점'과 현재 Three.js '로컬 원점(0,0,0)' 사이의 거리 차이를 저장합니다.
     * 이 값이 있어야만 수천 킬로미터 떨어진 곳에서도 정밀한 좌표 계산이 가능합니다.
     */
    originOffset,
    setOriginOffset,

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
      /**
       * 🚀 [초기화 스냅 적용]
       * 최초 원점 설정 시에도 projection.snapToGrid를 사용하여
       * 월드의 시작점을 4m 격자 눈금에 정확히 맞춥니다.
       */
      setOrigin(devicePos); // 내부에서 snapToGrid가 적용됨 (스토어 로직)

      const snappedInitial = projection.snapToGrid(devicePos);
      setOriginOffset(snappedInitial);

      setCenter(devicePos); // 센터는 부드러운 시작을 위해 소수점 유지
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

  // 레이캐스터 생성을 위한 Ref
  const raycaster = useRef(new THREE.Raycaster());
  const mouse = useRef(new THREE.Vector2());

  // --- [틸트 및 타겟 고정 Ref] ---
  const lastMousePos = useRef({ x: 0, y: 0 });
  const isRightMouseDown = useRef(false);

  useEffect(() => {
    const domElement = gl.domElement;

    const handleMouseDown = (e: MouseEvent) => {
      if (e.button === 2) {
        isRightMouseDown.current = true;
        // 🚀 현재 마우스 위치 저장
        lastMousePos.current = { x: e.clientX, y: e.clientY };
        domElement.style.cursor = 'grabbing';
      }
    };

    const handleMouseMove = (e: MouseEvent) => {
      if (!isRightMouseDown.current || !controlsRef.current) return;

      // 1. 실제 마우스 이동량 계산
      const deltaX = e.clientX - lastMousePos.current.x;
      const deltaY = e.clientY - lastMousePos.current.y;

      lastMousePos.current = { x: e.clientX, y: e.clientY };

      // 2. 조작 감도 설정
      const bearingSensitivity = 0.012; // 회전은 조금 더 시원하게
      const tiltSensitivity = 0.004; // 틸트는 세밀하게

      // 🚀 [핵심] 베어링 우선순위 로직
      // 좌우 이동량(absX)이 상하 이동량(absY)보다 훨씬 크면 틸트(deltaY)를 무시하거나 감쇄시킵니다.
      const absX = Math.abs(deltaX);
      const absY = Math.abs(deltaY);

      // 수평 회전 적용 (Bearing)
      const currentAzimuth = controlsRef.current.getAzimuthalAngle();
      // 베어링: 오른쪽으로 밀면 지도가 오른쪽으로 (지면 잡기)
      controlsRef.current.setAzimuthalAngle(
        currentAzimuth + deltaX * bearingSensitivity,
      );

      // 수직 틸트 적용 (Tilt)
      // 좌우로 빠르게 돌리는 중(absX > absY * 1.5)에는 틸트가 거의 발생하지 않도록 보정
      if (absY > absX * 0.7) {
        const currentPolar = controlsRef.current.getPolarAngle();
        // 틸트 이동량에 가중치를 두어 베어링 조작 중 의도치 않은 들림 방지
        const weightedDeltaY = deltaY * (absY / (absX + absY));

        // 틸트: 아래로 당기면 지도가 내 쪽으로 일어남 (지면 잡기)
        controlsRef.current.setPolarAngle(
          currentPolar - weightedDeltaY * tiltSensitivity,
        );
      }

      controlsRef.current.update();
    };

    const handleMouseUp = (e: MouseEvent) => {
      if (!isRightMouseDown.current) return;
      isRightMouseDown.current = false;
      domElement.style.cursor = 'auto';

      if (controlsRef.current) {
        mouse.current.set(0, 0);
        raycaster.current.setFromCamera(mouse.current, camera);

        // 🚀 1. 지형 레이캐스팅 시도
        const intersects = raycaster.current.intersectObjects(
          scene.children,
          true,
        );

        if (intersects.length > 0) {
          const newTarget = intersects[0].point;

          // 타겟 업데이트
          controlsRef.current.target.copy(newTarget);

          // 🚀 2. 절대 좌표로 변환하여 Store 업데이트 (Center 동기화)
          const absolutePos = {
            x: newTarget.x + originOffset.x,
            y: newTarget.y + originOffset.y,
            z: newTarget.z + originOffset.z,
          };
          setCenter(absolutePos);
        }

        controlsRef.current.update();
      }
    };

    // 컨텍스트 메뉴(우클릭 메뉴) 방지
    const handleContextMenu = (e: MouseEvent) => e.preventDefault();

    domElement.addEventListener('mousedown', handleMouseDown);
    window.addEventListener('mousemove', handleMouseMove);
    window.addEventListener('mouseup', handleMouseUp);
    domElement.addEventListener('contextmenu', handleContextMenu);

    return () => {
      domElement.removeEventListener('mousedown', handleMouseDown);
      window.removeEventListener('mousemove', handleMouseMove);
      window.removeEventListener('mouseup', handleMouseUp);
      domElement.removeEventListener('contextmenu', handleContextMenu);
    };
  }, [gl, camera, scene, setCenter]);

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
   * 📡 [좌표 변환 동기화]
   * 절대 좌표(center)에서 스냅된 오프셋(originOffset)을 빼서
   * Three.js 공간의 '로컬 좌표'를 산출합니다.
   */
  useEffect(() => {
    if (!center || !origin || !controlsRef.current) return;
    if ((controlsRef.current as any).active) return;

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
  }, [center, origin, originOffset]);

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
     * 🔄 [Floating Origin Reset - Snap 적용]
     * 카메라가 멀어지면 세상을 다시 (0,0,0) 근처로 당깁니다.
     * 이때 '스냅된 거리'만큼 이동해야 배경 격자가 튀지 않습니다.
     */
    if (camera.position.length() > RESET_THRESHOLD) {
      const rawOffset = controlsRef.current.target.clone();

      // 🚀 [리팩토링] 직접 계산 대신 공통 유틸리티 snapToGrid 사용
      const snappedStep = projection.snapToGrid(rawOffset);

      // 누적 오프셋 갱신 (이미 내부에서 한 번 더 스냅하므로 안전)
      const newTotalOffset = {
        x: originOffset.x + snappedStep.x,
        y: originOffset.y + snappedStep.y,
        z: originOffset.z + snappedStep.z,
      };
      setOriginOffset(newTotalOffset);

      // 세계 전체와 카메라를 정확히 '격자 단위'로 반대 방향 이동
      worldGroupRef.current.position.sub(snappedStep);
      camera.position.sub(snappedStep);
      controlsRef.current.target.sub(snappedStep);

      worldGroupRef.current.updateMatrixWorld(true);
      camera.updateMatrixWorld(true);

      if (flyState.current.active) {
        flyState.current.position.sub(snappedStep);
        flyState.current.lookAt.sub(snappedStep);
      }

      controlsRef.current.update();
      console.log('🌐 Grid-Snapped Origin Reset:', newTotalOffset);
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
        // RIGHT: THREE.MOUSE.ROTATE, // 🚀 중요: 기본 회전 로직을 꺼서 충돌 제거 // 오른쪽 클릭: 회전
      }}
      touches={{
        ONE: THREE.TOUCH.PAN, // 한 손가락: 이동
        // TWO: THREE.TOUCH.DOLLY_PAN, // 두 손가락: 핀치 줌 + 회전
        TWO: THREE.TOUCH.DOLLY_ROTATE, // 🚀 DOLLY_PAN 대신 DOLLY_ROTATE 권장 (모바일 지도 표준)
      }}
      // 회전 관련 세밀한 설정
      enableRotate={true} // 회전 활성화
      rotateSpeed={1} // 🚀 다시 1로 복구 (모바일 터치 회전을 위해 필요)
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
           * 📍 [절대 좌표 역산]
           * 로컬의 상대 좌표(t)에 '스냅된 절대 오프셋'을 더해
           * 정밀도가 살아있는 진짜 지구 주소를 추출합니다.
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
