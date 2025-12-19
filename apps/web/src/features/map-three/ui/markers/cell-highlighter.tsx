// 1. 외부 라이브러리 (External Libraries)
import { useMemo } from 'react';
import * as THREE from 'three';
import { useFrame, useThree } from '@react-three/fiber';

// 2. 프로젝트 공통 유틸 및 타입 (Shared)
import { projection } from '@/shared/lib/geo/projection';
import type { MeterPos } from '@/shared/types/map';

// 3. 기능 전용 상태 관리 (Feature Store)
import { useMapStore } from '../../model/map-store';

/**
 * 마우스가 가리키는 바닥의 격자(Voxel)를 찾아
 * 하이라이트 사각형을 표시하는 컴포넌트
 */
export function CellHighlighter() {
  const { raycaster, pointer, camera } = useThree();

  const center = useMapStore((state) => state.center);
  const setCenter = useMapStore((state) => state.setCenter);
  const hoveredPos = useMapStore((state) => state.hoveredPos);
  const setHoveredPos = useMapStore((state) => state.setHoveredPos);

  const groundPlane = useMemo(() => new THREE.Plane(), []); // 매 프레임 업데이트 예정
  const intersectionPoint = useMemo(() => new THREE.Vector3(), []);
  const centerPosition = useMemo(() => new THREE.Vector3(), []);

  const limitUnits = 100; // 현실 100m 제한 설정
  const gridSize = projection.GRID_SIZE;

  useFrame(() => {
    if (!center) return;

    // 1. [중요] 레이캐스팅 평면을 현재 중심 고도(y)로 맞춤
    // y + d = 0 형태이므로 constant는 -y입니다.
    groundPlane.setComponents(0, 1, 0, -center.y);

    // 1. 뷰 중심(center)을 Vector3로 설정
    centerPosition.set(center.x, center.y, center.z);

    centerPosition.set(center.x, center.y, center.z);
    raycaster.setFromCamera(pointer, camera);

    const intersect = raycaster.ray.intersectPlane(
      groundPlane,
      intersectionPoint,
    );

    if (intersect) {
      const distanceUnits = centerPosition.distanceTo(intersect);

      if (distanceUnits <= limitUnits) {
        const vX = Math.floor(intersect.x / gridSize) * gridSize;
        const vZ = Math.floor(intersect.z / gridSize) * gridSize;

        // 좌표가 바뀌었을 때만 setPos 호출 (성능 최적화)
        if (!hoveredPos || hoveredPos.x !== vX || hoveredPos.z !== vZ) {
          // 격자의 시작점(vX, vZ)과 현재 지형의 높이(center.y)를 저장
          setHoveredPos({ x: vX, y: center.y, z: vZ });
        }
      } else {
        setHoveredPos(null);
      }
    } else {
      setHoveredPos(null);
    }
  });

  const handleClick = (e: any) => {
    e.stopPropagation();
    if (!hoveredPos) return;

    // 클릭한 위치로 중심 이동
    // MapCameraController가 이 center 변경을 감지하고 flyTo를 실행합니다.
    setCenter(hoveredPos);
  };

  if (!hoveredPos) return null;

  // 렌더링 위치는 4m 칸의 중앙인 +2.0 보정, z-fighting 방지를 위해 살짝 위(0.02)에 배치
  const offset = gridSize / 2;

  return (
    <mesh
      // z-fighting 방지를 위해 y값에 미세한 오프셋(+0.05) 추가
      position={[
        hoveredPos.x + offset,
        hoveredPos.y + 0.05,
        hoveredPos.z + offset,
      ]}
      rotation={[-Math.PI / 2, 0, 0]}
      onClick={handleClick}
      onPointerOver={() => (document.body.style.cursor = 'pointer')}
      onPointerOut={() => (document.body.style.cursor = 'auto')}
    >
      {/* 격자 크기에 비례하는 2.5% 여백(padding)을 주어 격자 간 구분을 명확히 함 */}
      <planeGeometry args={[gridSize * 0.975, gridSize * 0.975]} />
      <meshBasicMaterial
        color="#00ff00"
        transparent
        opacity={0.3}
        depthWrite={false} // 다른 오브젝트와 겹칠 때 깜빡임 방지
      />
    </mesh>
  );
}
