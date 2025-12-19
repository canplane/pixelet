// 1. 외부 라이브러리 (External Libraries)
import { useMemo, useRef } from 'react';
import * as THREE from 'three';
import { useFrame } from '@react-three/fiber';

// 2. 엔티티 및 상태 관리 (Entities)
// 기기 위치 정보는 지도를 넘어선 공통 엔티티로 유지됩니다.
import { useDeviceLocationStore } from '@/entities/device-location/model/device-location.store';

// 3. 기능 전용 상태 관리 (Feature Store)
import { useMapStore } from '../../model/map-store';

export function UserMarker() {
  const { location, gpsHeading, compassHeading, accuracy } =
    useDeviceLocationStore();
  const { project } = useMapStore();

  const compassGroupRef = useRef<THREE.Group>(null);
  const gpsGroupRef = useRef<THREE.Group>(null);

  /**
   * 1. 위치 정보 변환
   * location.alt가 포함된 절대 미터 좌표(MeterPos)를 생성합니다.
   */
  const position = useMemo(
    () => (location ? project(location) : null),
    [location, project],
  );

  /**
   * 2. 회전 애니메이션 보간 (Slerp)
   * Three.js는 시계 반대 방향이 양수이므로 도(deg)를 라디안으로 바꿀 때 마이너스(-)를 적용합니다.
   */
  useFrame(() => {
    const toRad = (deg: number) => -(deg * Math.PI) / 180;

    if (compassGroupRef.current && compassHeading !== null) {
      const targetQuat = new THREE.Quaternion().setFromEuler(
        new THREE.Euler(0, toRad(compassHeading), 0),
      );
      compassGroupRef.current.quaternion.slerp(targetQuat, 0.1);
    }

    if (gpsGroupRef.current && gpsHeading !== null) {
      const targetQuat = new THREE.Quaternion().setFromEuler(
        new THREE.Euler(0, toRad(gpsHeading), 0),
      );
      gpsGroupRef.current.quaternion.slerp(targetQuat, 0.1);
    }
  });

  if (!position) return null;

  /**
   * 3. 물리적 크기 설정 (1 Unit = 1 Meter)
   * 더 이상 mapConverter를 사용하지 않고 실제 미터 값을 그대로 입력합니다.
   */
  const markerRadius = 0.6; // 마커 지름 약 1.2m
  const coneRadius = 0.3;
  const coneHeight = 0.8;
  const visualHeight = 0.2; // 바닥에서 살짝 띄움 (z-fighting 방지)

  return (
    // position.y에 실제 GPS 고도값이 반영됩니다.
    <group position={[position.x, position.y, position.z]}>
      {/* 1. 정확도 범위 (GPS Accuracy Circle) */}
      <mesh rotation={[-Math.PI / 2, 0, 0]}>
        <circleGeometry args={[accuracy || 0, 32]} />
        <meshBasicMaterial color="#4285F4" transparent opacity={0.1} />
      </mesh>

      {/* 2. 시선 방향 부채꼴 (Compass / Field of View) */}
      {compassHeading !== null && (
        <group ref={compassGroupRef}>
          <mesh
            rotation={[-Math.PI / 2, 0, 0]}
            position={[0, 0.05, -2]} // 전방 2m 지점 중심
          >
            <ringGeometry
              args={[
                0,
                4, // 부채꼴 반지름 4m
                32,
                1,
                Math.PI * (0.5 - 0.1),
                Math.PI * 0.2,
              ]}
            />
            <meshBasicMaterial
              color="#4285F4"
              transparent
              opacity={0.3}
              side={THREE.DoubleSide}
            />
          </mesh>
        </group>
      )}

      {/* 3. 이동 방향 화살표 (GPS Heading Arrow) */}
      {gpsHeading !== null && (
        <group ref={gpsGroupRef}>
          <mesh
            position={[0, visualHeight * 2.5, -1.2]}
            rotation={[-Math.PI / 2, 0, 0]}
          >
            <coneGeometry args={[coneRadius, coneHeight, 3]} />
            <meshBasicMaterial color="white" />
          </mesh>
        </group>
      )}

      {/* 4. 내 위치 중심 점 (Sphere) */}
      <mesh position={[0, visualHeight, 0]}>
        <sphereGeometry args={[markerRadius, 16, 16]} />
        <meshBasicMaterial color="#4285F4" />
      </mesh>
    </group>
  );
}
