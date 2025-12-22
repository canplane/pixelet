// 1. 외부 라이브러리
import { useMemo, useRef } from 'react';
import * as THREE from 'three';
import { useFrame, useThree } from '@react-three/fiber';
import { Line } from '@react-three/drei';

// 2. 프로젝트 공통
import { useMapStore } from '../../model/map-store';
import { projection } from '@/shared/lib/geo/projection';
import { THREE_CONFIG } from '../../model/three-config';

const { GRID_SIZE } = projection;
const { FOG_FAR, FOG_NEAR } = THREE_CONFIG.RENDERING;

export function CellHighlighter() {
  const { raycaster, pointer, camera } = useThree();
  const { center, setCenter, hoveredPos, setHoveredPos } = useMapStore();

  // 🚀 [추가] 오프셋 구독
  const originOffset = useMapStore((state) => state.originOffset);

  const groundPlane = useMemo(() => new THREE.Plane(), []);
  const intersectionPoint = useMemo(() => new THREE.Vector3(), []);
  const localCenterVec = useMemo(() => new THREE.Vector3(), []);

  const limitUnits = 100;

  const offset = GRID_SIZE / 2;
  const localGroupRef = useRef<THREE.Group>(null); // 최상위 위치 보정용
  const aniGroupRef = useRef<THREE.Group>(null); // 위아래 애니메이션용

  // 🚀 1. 단정한 라운딩 프레임 경로 (Radius 0.15)
  const roundedPoints = useMemo(() => {
    const size = GRID_SIZE * 0.95;
    const radius = size * 0.15; // 절반으로 줄인 라운딩
    const pts = [];
    const steps = 8;
    for (let i = 0; i <= steps * 4; i++) {
      const angle = (i / (steps * 4)) * Math.PI * 2;
      const px =
        (Math.cos(angle) > 0 ? 1 : -1) * (size / 2 - radius) +
        Math.cos(angle) * radius;
      const py =
        (Math.sin(angle) > 0 ? 1 : -1) * (size / 2 - radius) +
        Math.sin(angle) * radius;
      pts.push(new THREE.Vector3(px, py, 0));
    }
    return pts;
  }, [GRID_SIZE]);

  useFrame((state) => {
    // 1. 위치 보정 (Floating Origin 실시간 대응)
    if (localGroupRef.current && hoveredPos) {
      // 렌더링 루프 안에서 직접 위치 수정 (React 리렌더링 없이)
      localGroupRef.current.position.set(
        hoveredPos.x - originOffset.x + offset,
        0,
        hoveredPos.z - originOffset.z + offset,
      );
    }

    if (!center) return;

    // 2. 레이캐스팅 로직 (로컬 0점 기준)
    groundPlane.setComponents(0, 1, 0, 0);
    localCenterVec.set(center.x - originOffset.x, 0, center.z - originOffset.z);
    raycaster.setFromCamera(pointer, camera);
    const intersect = raycaster.ray.intersectPlane(
      groundPlane,
      intersectionPoint,
    );

    // 🚀 [수정] 제한 거리를 CONFIG의 안개 끝 지점(FOG_FAR)으로 설정
    if (intersect && localCenterVec.distanceTo(intersect) <= FOG_FAR) {
      /**
       * 🚀 [좌표 역산]
       * 마우스가 찍은 곳은 '로컬'입니다. 여기에 오프셋을 더해 '절대 좌표'로 변환 후 저장합니다.
       */
      const absX = intersect.x + originOffset.x;
      const absZ = intersect.z + originOffset.z;
      const vX = Math.floor(absX / GRID_SIZE) * GRID_SIZE;
      const vZ = Math.floor(absZ / GRID_SIZE) * GRID_SIZE;

      if (!hoveredPos || hoveredPos.x !== vX || hoveredPos.z !== vZ) {
        setHoveredPos({ x: vX, y: center.y, z: vZ });
      }

      // 🚀 [추가] 거리 기반 투명도 계산 (안개와 동기화)
      if (localGroupRef.current) {
        const dist = camera.position.distanceTo(localGroupRef.current.position);
        const opacity = THREE.MathUtils.mapLinear(
          dist,
          FOG_NEAR,
          FOG_FAR,
          1,
          0,
        );
        localGroupRef.current.visible = opacity > 0.05;
        // 필요 시 group 내부의 재질 투명도 조절 로직 추가 가능
      }
    } else if (hoveredPos) {
      setHoveredPos(null);
    }

    // 3. 위아래 숨쉬기 애니메이션
    if (aniGroupRef.current) {
      const t = state.clock.getElapsedTime();
      // Y축 애니메이션만 수행 (X, Z는 부모 group에서 보정됨)
      aniGroupRef.current.position.y = 0.04 + Math.sin(t * 2) * 0.015;
    }
  });

  if (!hoveredPos) return null;

  return (
    // 🚀 ref={localGroupRef}를 할당하여 useFrame에서 제어하게 합니다.
    // 초기 position은 0으로 두어도 useFrame이 즉시 잡아줍니다.
    <group ref={localGroupRef}>
      <group ref={aniGroupRef} rotation={[-Math.PI / 2, 0, 0]}>
        {/* 🚀 3. 클릭 영역 (투명) */}
        <mesh
          onClick={(e) => {
            e.stopPropagation();
            setCenter(hoveredPos);
          }}
          onPointerOver={() => (document.body.style.cursor = 'pointer')}
          onPointerOut={() => (document.body.style.cursor = 'auto')}
        >
          <planeGeometry args={[GRID_SIZE, GRID_SIZE]} />
          <meshBasicMaterial transparent opacity={0} depthWrite={false} />
        </mesh>

        {/* 🚀 4. 파스텔 프레임 라인 */}
        <Line
          points={roundedPoints}
          color="#FFB7B2"
          lineWidth={2}
          transparent
          opacity={0.6}
        />

        {/* 🚀 5. 모서리 포인트들 */}
        {[
          [-1, -1],
          [-1, 1],
          [1, -1],
          [1, 1],
        ].map(([mx, my], i) => (
          <mesh
            key={i}
            position={[
              mx * ((GRID_SIZE / 2) * 0.75),
              my * ((GRID_SIZE / 2) * 0.75),
              0,
            ]}
          >
            <circleGeometry args={[0.04, 16]} />
            <meshBasicMaterial color="#FFB7B2" transparent opacity={0.5} />
          </mesh>
        ))}
      </group>

      {/* 🚀 6. 바닥의 아주 은은한 가이드 (그림자 대용) */}
      <mesh rotation={[-Math.PI / 2, 0, 0]} position={[0, 0.01, 0]}>
        <planeGeometry args={[GRID_SIZE * 0.9, GRID_SIZE * 0.9]} />
        <meshBasicMaterial
          color="#FFB7B2"
          transparent
          opacity={0.05}
          depthWrite={false}
        />
      </mesh>
    </group>
  );
}
