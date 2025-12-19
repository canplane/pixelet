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
import { useDeviceLocationStore } from '@/entities/device-location/model/device-location.store';

const { DEFAULT_DISTANCE, DEFAULT_POLAR_ANGLE } = THREE_CONFIG.CAMERA;

export function MapCameraController() {
  const { camera } = useThree();
  const controlsRef = useRef<OrbitControlsImpl>(null);
  const isInitialized = useRef(false); // 초기 로딩 여부 체크

  // 기기 실제 GPS 위치
  const { location: deviceLocation, status } = useDeviceLocationStore();

  // Store에서 필요한 상태와 액션 구독
  const center = useMapStore((state) => state.center);
  const setCenter = useMapStore((state) => state.setCenter);
  const origin = useMapStore((state) => state.origin);
  const setOrigin = useMapStore((state) => state.setOrigin);
  const project = useMapStore((state) => state.project);
  const setViewState = useMapStore((state) => state.setViewState);
  const resetSignal = useMapStore((state) => state.cameraResetSignal);

  // 3. 통합 위치 및 원점 관리 (절대 좌표계 초기화)
  useEffect(() => {
    if (status !== 'ready' || !deviceLocation) return;

    // 초기 상태: 기준점이 없으면 현재 위치를 기준점으로 잡음
    if (!origin) {
      setOrigin(deviceLocation);
      // 초기 카메라 중심을 내 위치의 절대 미터 좌표로 설정
      const initialCenter = project(deviceLocation);
      setCenter(initialCenter); // 카메라를 수백만 미터 지점으로 즉시 보냄
      return;
    }

    /** * [절대 좌표 시스템 전환 알림]
     * 이제 origin과의 거리를 비교하여 원점을 리셋하던 로직은 필요 없습니다.
     * 모든 좌표가 Web Mercator 미터 단위로 직접 관리되기 때문입니다.
     * 만약 "내 위치 추적 모드"라면 아래 주석을 해제하세요.
     */
    // setCenter(project(deviceLocation));
  }, [deviceLocation, origin, status, project, setOrigin, setCenter]);

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
   * [위치 동기화 로직]
   * 초기 로딩 시에는 syncCamera(순간이동),
   * 이후 데이터 업데이트 시에는 flyTo(부드러운 이동)를 사용합니다.
   */
  useEffect(() => {
    if (!center || !origin || (controlsRef.current as any).active) return;

    const currentKey = JSON.stringify(center);
    if (lastProcessedCenter.current === currentKey) return;
    lastProcessedCenter.current = currentKey;

    // 현재 거리 계산
    const currentDist = camera.position.distanceTo(
      (controlsRef.current as any).target,
    );
    const finalDistance = currentDist > 1 ? currentDist : DEFAULT_DISTANCE;

    if (!isInitialized.current) {
      // 1. [초기 로딩] 앱이 처음 켜질 때는 순간이동!
      syncCamera(center, finalDistance);
      isInitialized.current = true; // 초기화 완료 표시
    } else {
      // 2. [이후 업데이트] GPS 이동이나 센터 변경 시에는 부드럽게 비행!
      flyTo(center, finalDistance);
    }
  }, [center, origin]);

  // 거리 리셋 감지
  useEffect(() => {
    if (!controlsRef.current || resetSignal.distance === 0) return;
    flyTo(controlsRef.current.target, DEFAULT_DISTANCE); // 현재 각도는 유지한 채 거리만 리셋
  }, [resetSignal.distance]);

  // 방향 리셋 감지
  useEffect(() => {
    if (!controlsRef.current || resetSignal.orientation === 0) return;
    controlsRef.current.setAzimuthalAngle(0);
    controlsRef.current.setPolarAngle(DEFAULT_POLAR_ANGLE);
    const currentDist = camera.position.distanceTo(controlsRef.current.target); // 현재 거리는 유지한 채 각도만 리셋
    flyTo(controlsRef.current.target, currentDist);
  }, [resetSignal.orientation]);

  // 3. 외부 위치(Store/GPS) 동기화
  useEffect(() => {
    const targetCenter = center;
    if (!targetCenter || !origin || (controlsRef.current as any).active) return;

    const currentKey = JSON.stringify(targetCenter);
    if (lastProcessedCenter.current === currentKey) return;
    lastProcessedCenter.current = currentKey;

    // 현재 사용자가 조절한 줌(거리)을 유지하며 중심점만 이동
    const currentDist = camera.position.distanceTo(
      (controlsRef.current as any).target,
    );
    const finalDistance = currentDist > 1 ? currentDist : DEFAULT_DISTANCE;

    flyTo(targetCenter, finalDistance);
  }, [center, origin]);

  // --- 실시간 카메라 상태 업데이트 (ScaleBar, Compass용) ---
  useFrame((state) => {
    if (!controlsRef.current) return;

    // --- 부드러운 비행 처리 (Fly Animation) ---
    if (flyState.current.active) {
      const lerpFactor = 0.1; // 0.1 ~ 0.2 사이가 쫀득합니다.
      camera.position.lerp(flyState.current.position, lerpFactor);
      controlsRef.current.target.lerp(flyState.current.lookAt, lerpFactor);
      controlsRef.current.update();

      if (camera.position.distanceTo(flyState.current.position) < 0.01) {
        flyState.current.active = false;
      }
    }

    // --- 상태 업데이트 보고 ---
    const now = state.clock.getElapsedTime();
    if (now - lastUpdateTime.current < 0.05) return;

    const distance = camera.position.distanceTo(controlsRef.current.target); // 직선 거리 (줌 레벨)
    const altitude = camera.position.y - controlsRef.current.target.y; // 실제 수직 고도 (Altitude)

    const vector = new THREE.Vector3(0, 0, -1).applyQuaternion(
      camera.quaternion,
    );
    const bearing = Math.atan2(vector.x, vector.z) * (180 / Math.PI);
    const pitch = (controlsRef.current.getPolarAngle() * 180) / Math.PI;

    const distanceDiff = Math.abs(lastUpdate.current.distance - distance);
    const bearingDiff = Math.abs(lastUpdate.current.bearing - bearing);

    // 변화량이 일정 수준 이상일 때만 상위로 전달 (성능 최적화)
    if (distanceDiff > distance * 0.005 || bearingDiff > 0.1) {
      lastUpdate.current = { distance, bearing };
      lastUpdateTime.current = now;

      // Store의 viewState 실시간 업데이트
      setViewState({
        distance,
        altitude,
        bearing,
        pitch,
      });
    }
  });

  return (
    <OrbitControls
      ref={controlsRef}
      // 사용자가 조작을 시작하면 애니메이션 중단
      onStart={() => {
        flyState.current.active = false;
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
      rotateSpeed={0.5} // 회전 속도 (너무 빠르면 낮추세요)
      // 댐핑(관성) 설정
      enableDamping={true}
      dampingFactor={0.05} // 너무 낮으면 줌 반응이 느려짐
      // 기타 거리 제한 (축척 바가 10m ~ 1000m 사이에서만 움직이도록 제한)
      minDistance={10}
      maxDistance={1000} // 너무 크면 정밀도가 떨어질 수 있음
      // 이동 및 회전 제한
      screenSpacePanning={false} // 드래그 시 카메라가 화면 방향이 아닌 '지면(바닥)'을 따라 움직이게 합니다.
      maxPolarAngle={Math.PI / 2.1} // 수직 회전의 아래쪽 한계 (90도 직전)
      minPolarAngle={0} // 수직 회전의 위쪽 한계 (0도 근처)
      onEnd={() => {
        if (controlsRef.current) {
          const t = controlsRef.current.target;
          const newPos: MeterPos = { x: t.x, y: t.y, z: t.z };

          // [중요] 사용자가 직접 움직인 좌표임을 기록하여 외부 업데이트 간섭 방지
          lastProcessedCenter.current = JSON.stringify(newPos);

          // Store 업데이트
          setCenter(newPos);
        }
      }}
    />
  );
}
