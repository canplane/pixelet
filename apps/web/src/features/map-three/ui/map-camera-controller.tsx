import { useEffect, useMemo, useRef } from 'react';
import * as THREE from 'three';
import { useFrame, useThree } from '@react-three/fiber';
import { OrbitControls } from '@react-three/drei';
import { THREE_CONFIG } from '@/features/map-three/model/three-config';
import { mapConverter } from '@/shared/lib/geo/map-converter'; // 컨버터 임포트
import type { RelativeWorldPos } from '@/shared/types/map';
import { useMapStore } from '@/entities/map/model/map.store';

interface Props {
  targetPos: RelativeWorldPos | null; // 검색 등을 통한 목표 좌표
  onMapMove?: (pos: RelativeWorldPos) => void;
  onUpdate: (camData: { dist: number; rot: number }) => void;
}

export function MapCameraController({ targetPos, onMapMove, onUpdate }: Props) {
  const { camera, size } = useThree();
  const controlsRef = useRef<any>(null);
  const lastProcessedTarget = useRef<string>('');
  const { origin } = useMapStore();

  const lastUpdate = useRef({ dist: 0, rot: 0 });
  const lastUpdateTime = useRef(0);

  useFrame((state) => {
    if (controlsRef.current) {
      const now = state.clock.getElapsedTime();
      if (now - lastUpdateTime.current < 0.05) return;

      const dist = camera.position.distanceTo(controlsRef.current.target);
      const vector = new THREE.Vector3(0, 0, -1).applyQuaternion(
        camera.quaternion,
      );
      const rot = Math.atan2(vector.x, vector.z) * (180 / Math.PI);

      const distDiff = Math.abs(lastUpdate.current.dist - dist);
      const rotDiff = Math.abs(lastUpdate.current.rot - rot);

      if (distDiff > dist * 0.005 || rotDiff > 0.1) {
        lastUpdate.current = { dist, rot };
        lastUpdateTime.current = now;
        setTimeout(() => onUpdate({ dist, rot }), 0);
      }
    }
  });

  useEffect(() => {
    if (!targetPos || !origin) return;

    const currentKey = JSON.stringify(targetPos);

    // 1. [가드] 내가 방금 보낸 좌표라면 무시
    if (lastProcessedTarget.current === currentKey) return;

    // 2. [가드] 사용자가 조작 중(드래그, 줌)인 동안에는 외부 업데이트를 차단
    // controls.enabled가 아닌, 내부 상태인 'pointer-down' 여부를 체크하는 것이 정확합니다.
    if (controlsRef.current?.active) return;

    lastProcessedTarget.current = currentKey;

    // 3. [거리 유지 로직] 줌 튕김을 막기 위해 현재 카메라 거리를 가져옵니다.
    // 만약 "검색 이동"처럼 특정 축척으로 고정하고 싶다면 targetDistance를 그대로 쓰시고,
    // 아니라면 현재 거리를 유지하는 것이 튕김이 없습니다.
    const currentDist = camera.position.distanceTo(
      controlsRef.current?.target || new THREE.Vector3(),
    );

    const unitsForMeters = mapConverter.metersToUnits(
      THREE_CONFIG.CAMERA.TARGET_METERS,
    );
    const visibleHeight =
      (unitsForMeters / THREE_CONFIG.CAMERA.REFERENCE_PIXELS) * size.height;
    const fovRad = THREE_CONFIG.CAMERA.FOV * (Math.PI / 180);
    const targetDistance = visibleHeight / 2 / Math.tan(fovRad / 2);

    // [선택] 0이면(초기화 시) targetDistance 사용, 아니면 현재 거리(줌) 유지
    const finalDistance = currentDist > 1 ? currentDist : targetDistance;

    const radTilt = (THREE_CONFIG.CAMERA.DEFAULT_TILT * Math.PI) / 180;
    const posY = finalDistance * Math.sin(radTilt);
    const posZ = finalDistance * Math.cos(radTilt);

    camera.position.set(targetPos[0], targetPos[1] + posY, targetPos[2] + posZ);

    if (controlsRef.current) {
      controlsRef.current.target.set(...targetPos);
      controlsRef.current.update();
    }
  }, [targetPos, origin]);

  // --- 축척 바 기준 거리 계산 로직 ---
  const { minUnitDistance, maxUnitDistance } = useMemo(() => {
    const fovRad = THREE_CONFIG.CAMERA.FOV * (Math.PI / 180);

    // 1. 축척 바 픽셀 대비 화면 높이 비율
    const barToScreenRatio = size.height / THREE_CONFIG.CAMERA.REFERENCE_PIXELS;

    // 2. 각 목표 미터(10m, 1000m)를 지도 유닛으로 변환
    // [수정] mapConverter를 사용하여 SCALE_FACTOR 대응
    const unit10m = mapConverter.metersToUnits(10);
    const unit1000m = mapConverter.metersToUnits(1000);

    // 3. 축척 바가 해당 미터를 가리킬 때의 visibleHeight(3D 유닛 높이) 계산
    const visibleHeight10m = unit10m * barToScreenRatio;
    const visibleHeight1000m = unit1000m * barToScreenRatio;

    // 4. visibleHeight를 카메라 거리(distance)로 역산
    const minD = visibleHeight10m / 2 / Math.tan(fovRad / 2);
    const maxD = visibleHeight1000m / 2 / Math.tan(fovRad / 2);

    return { minUnitDistance: minD, maxUnitDistance: maxD };
  }, [size.height]); // 화면 크기가 바뀔 때만 재계산

  return (
    <OrbitControls
      ref={controlsRef}
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
      minDistance={minUnitDistance}
      maxDistance={maxUnitDistance} // 너무 크면 정밀도가 떨어질 수 있음
      // 이동 및 회전 제한
      screenSpacePanning={false} // 드래그 시 카메라가 화면 방향이 아닌 '지면(바닥)'을 따라 움직이게 합니다.
      maxPolarAngle={Math.PI / 2.1} // 수직 회전의 아래쪽 한계 (90도 직전)
      minPolarAngle={0} // 수직 회전의 위쪽 한계 (0도 근처)
      onEnd={() => {
        if (controlsRef.current) {
          const t = controlsRef.current.target;
          const newPos: RelativeWorldPos = [t.x, t.y, t.z];

          // [중요] 내가 직접 움직인 좌표임을 기록하여 useEffect의 강제 리셋 방지
          lastProcessedTarget.current = JSON.stringify(newPos);
          onMapMove?.(newPos);
        }
      }}
    />
  );
}
