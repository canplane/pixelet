import { useMemo, useRef } from 'react';
import { useFrame } from '@react-three/fiber';
import * as THREE from 'three';
import { useDeviceLocationStore } from '@/entities/device-location/model/device-location.store';
import { useMapStore } from '@/entities/map/model/map.store';
import { mapConverter } from '@/shared/lib/geo/map-converter'; // 컨버터 임포트

export function UserMarker() {
  const { lngLat, gpsHeading, compassHeading, accuracy } =
    useDeviceLocationStore();
  const { project } = useMapStore();

  const compassGroupRef = useRef<THREE.Group>(null);
  const gpsGroupRef = useRef<THREE.Group>(null);

  // 1. 위경도를 3D 상대 좌표로 변환 (project 함수 내부에 이미 SCALE_FACTOR가 반영되어 있음)
  const position = useMemo(
    () => (lngLat ? project(lngLat) : null),
    [lngLat, project],
  );

  // 2. 매 프레임마다 회전값 보간 애니메이션 수행
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

  // --- [중요] 모든 물리적 크기를 유닛 단위로 변환 ---
  const unitAccuracy = mapConverter.metersToUnits(accuracy || 0);
  const unitMarkerSize = mapConverter.metersToUnits(0.6); // 마커 지름 약 1.2m
  const unitConeRadius = mapConverter.metersToUnits(0.3);
  const unitConeHeight = mapConverter.metersToUnits(0.8);
  const unitVisualHeight = mapConverter.metersToUnits(0.2); // 공중에 살짝 띄움

  return (
    <group position={[position[0], 0, position[2]]}>
      {/* 1. 정확도 범위 (GPS Accuracy) - 실제 미터 반경 반영 */}
      <mesh rotation={[-Math.PI / 2, 0, 0]}>
        <circleGeometry args={[unitAccuracy, 32]} />
        <meshBasicMaterial color="#4285F4" transparent opacity={0.1} />
      </mesh>

      {/* 2. 시선 방향 부채꼴 (Compass) */}
      {compassHeading !== null && (
        <group ref={compassGroupRef}>
          {/* 부채꼴 거리를 4m 정도로 설정 */}
          <mesh
            rotation={[-Math.PI / 2, 0, 0]}
            position={[0, 0.05, -mapConverter.metersToUnits(2)]}
          >
            <ringGeometry
              args={[
                0,
                mapConverter.metersToUnits(4),
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

      {/* 3. 이동 방향 화살표 (GPS Heading) */}
      {gpsHeading !== null && (
        <group ref={gpsGroupRef}>
          <mesh
            position={[
              0,
              unitVisualHeight * 2.5,
              -mapConverter.metersToUnits(1.2),
            ]}
            rotation={[-Math.PI / 2, 0, 0]}
          >
            <coneGeometry args={[unitConeRadius, unitConeHeight, 3]} />
            <meshBasicMaterial color="white" />
          </mesh>
        </group>
      )}

      {/* 4. 중심 점 */}
      <mesh position={[0, unitVisualHeight, 0]}>
        <sphereGeometry args={[unitMarkerSize, 16, 16]} />
        <meshBasicMaterial color="#4285F4" />
      </mesh>
    </group>
  );
}
