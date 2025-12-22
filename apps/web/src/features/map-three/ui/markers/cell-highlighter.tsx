// 1. 외부 라이브러리
import { useMemo, useRef } from 'react';
import * as THREE from 'three';
import { useFrame, useThree } from '@react-three/fiber';
import { Line } from '@react-three/drei';

// 2. 프로젝트 공통
import { useMapNavigation } from '../../model/use-map-navigation';
import { projection } from '@/shared/lib/geo/projection';
import { THREE_CONFIG } from '../../model/three-config';
import { MAP_THEMES } from '../../model/map-theme'; // 🚀 테마 데이터 임포트

const { GRID_SIZE, HALF_GRID } = projection;
const { FOG_FAR, FOG_NEAR } = THREE_CONFIG.RENDERING;

export function CellHighlighter() {
  const { raycaster, pointer, camera } = useThree();
  const {
    center,
    setCenter,
    originOffset,
    hoveredPos,
    setHoveredPos,
    themeKey,
  } = useMapNavigation();

  // 🚀 현재 테마 정보 가져오기
  const targetTheme = MAP_THEMES[themeKey];
  const themeColor = targetTheme.grid.section; // 하이라이트 메인 컬러 (격자의 굵은 선 색상을 추종)

  // 1. 레이캐스팅 평면 높이 보정 (지형 높이에 맞춤)
  const groundPlane = useMemo(
    // 🚀 지형이 0.005~0.15 사이에 있으므로, 0.1 정도에서 검출하는 것이 정확합니다.
    () => new THREE.Plane(new THREE.Vector3(0, 1, 0), -0.1),
    [],
  );

  const intersectionPoint = useMemo(() => new THREE.Vector3(), []);

  const localGroupRef = useRef<THREE.Group>(null);
  const aniGroupRef = useRef<THREE.Group>(null);

  // 1. 라운딩 프레임 경로 계산
  const roundedPoints = useMemo(() => {
    const size = GRID_SIZE * 0.95;
    const radius = size * 0.15;
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
  }, []);

  useFrame((state) => {
    if (localGroupRef.current && hoveredPos) {
      localGroupRef.current.position.set(
        hoveredPos.x - originOffset.x,
        0.16, // 🚀 도로(0.15)보다 아주 살짝 위로 올림 (Z-fighting 방지)
        hoveredPos.z - originOffset.z,
      );
    }

    if (!center) return;

    raycaster.setFromCamera(pointer, camera);
    const intersect = raycaster.ray.intersectPlane(
      groundPlane,
      intersectionPoint,
    );

    if (intersect) {
      const distToCamera = camera.position.distanceTo(intersect);

      // 🚀 [수정] FOG_FAR가 너무 짧으면 하이라이터가 일찍 사라집니다. 여유를 둡니다.
      if (distToCamera <= FOG_FAR * 1.5) {
        const absolutePoint = {
          x: intersect.x + originOffset.x,
          y: 0,
          z: intersect.z + originOffset.z,
        };

        const snapped = projection.snapToGrid(absolutePoint);

        if (
          !hoveredPos ||
          hoveredPos.x !== snapped.x ||
          hoveredPos.z !== snapped.z
        ) {
          setHoveredPos({ x: snapped.x, y: center.y, z: snapped.z });
        }

        if (localGroupRef.current) {
          // 안개 보정 수치 완화
          const opacity = THREE.MathUtils.mapLinear(
            distToCamera,
            FOG_NEAR,
            FOG_FAR * 1.2,
            1,
            0,
          );
          localGroupRef.current.visible = opacity > 0.01;
        }
      } else {
        setHoveredPos(null);
      }
    }

    // 3. 숨쉬기 애니메이션
    if (aniGroupRef.current) {
      const t = state.clock.getElapsedTime();
      aniGroupRef.current.position.y = 0.05 + Math.sin(t * 2.5) * 0.02;
    }
  });

  if (!hoveredPos) return null;

  return (
    <group ref={localGroupRef}>
      <group
        ref={aniGroupRef}
        rotation={[-Math.PI / 2, 0, 0]}
        position={[HALF_GRID, 0.02, HALF_GRID]} // 🚀 바닥에서 살짝 띄움
      >
        {/* 클릭 감지용 투명 메쉬 */}
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

        {/* 🚀 renderOrder를 높여 다른 지형물 위에 확실히 그리게 함 */}
        <Line
          points={roundedPoints}
          color={themeColor}
          lineWidth={3} // 🚀 가독성을 위해 조금 더 두껍게
          transparent
          opacity={0.8}
          renderOrder={100}
        />

        {/* 🚀 [테마 적용] 모서리 도트 포인트 */}
        {[
          [-1, -1],
          [-1, 1],
          [1, -1],
          [1, 1],
        ].map(([mx, my], i) => (
          <mesh key={i} renderOrder={101}>
            <circleGeometry args={[0.06, 16]} />
            <meshBasicMaterial color={themeColor} />
          </mesh>
        ))}
      </group>

      {/* 🚀 [테마 적용] 바닥 가이드 영역 */}
      <mesh
        rotation={[-Math.PI / 2, 0, 0]}
        position={[HALF_GRID, 0.01, HALF_GRID]}
        renderOrder={99}
      >
        <planeGeometry args={[GRID_SIZE * 0.95, GRID_SIZE * 0.95]} />
        <meshBasicMaterial
          color={themeColor}
          transparent
          opacity={0.15} // 🚀 0.08은 너무 연해서 안 보일 수 있음
          depthWrite={false}
        />
      </mesh>
    </group>
  );
}
