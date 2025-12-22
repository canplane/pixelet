// 1. 외부 라이브러리
import { useMemo, useRef } from 'react';
import * as THREE from 'three';
import { useFrame, useThree } from '@react-three/fiber';
import { Line } from '@react-three/drei';

// 2. 프로젝트 공통
import { useMapNavigation } from '../../model/use-map-navigation';
import { VOXEL_CONFIG } from '../../model/voxel-config';
import { THREE_CONFIG } from '../../model/three-config';
import { MAP_THEMES } from '../../model/map-theme'; // 🚀 테마 데이터 임포트

const { FOG_DENSITY, FADE_END } = THREE_CONFIG.RENDERING;

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
    const size = VOXEL_CONFIG.SIZE * 0.95;
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
        hoveredPos.x,
        0.16, // 🚀 도로(0.15)보다 아주 살짝 위로 올림 (Z-fighting 방지)
        hoveredPos.z,
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

      // 🚀 [수정] FADE_END가 너무 짧으면 하이라이터가 일찍 사라집니다. 여유를 둡니다.
      if (distToCamera <= FADE_END * 1.5) {
        const absolutePoint = {
          x: intersect.x + originOffset.x,
          y: 0, // 🚀 이 값이 있어야 snapToVoxel이 정상적으로 y축 스냅을 계산합니다.
          z: intersect.z + originOffset.z,
        };

        const snapped = VOXEL_CONFIG.snapToVoxel(absolutePoint);

        if (
          !hoveredPos ||
          hoveredPos.x !== snapped.x ||
          hoveredPos.z !== snapped.z
        ) {
          setHoveredPos({ x: snapped.x, y: 0, z: snapped.z });
        }

        if (localGroupRef.current) {
          // 🚀 2. UserMarker와 동일한 지수 안개 공식 적용
          const visibility = Math.exp(-Math.pow(distToCamera * FOG_DENSITY, 2));

          // 🚀 3. 단순 visible 여부뿐만 아니라 투명도(opacity)에도 직접 반영
          // 이렇게 하면 안개 속에 하이라이터가 더 자연스럽게 스며듭니다.
          localGroupRef.current.visible = visibility > 0.01;

          // (선택사항) 내부 메테리얼들의 opacity를 visibility와 연동하면 금상첨화입니다.
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
        position={[
          VOXEL_CONFIG.toCenter(0),
          VOXEL_CONFIG.STEP + 0.125,
          VOXEL_CONFIG.toCenter(0),
        ]} // 🚀 바닥에서 살짝 띄움
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
          <planeGeometry args={[VOXEL_CONFIG.SIZE, VOXEL_CONFIG.SIZE]} />
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
        position={[
          VOXEL_CONFIG.toCenter(0),
          VOXEL_CONFIG.STEP,
          VOXEL_CONFIG.toCenter(0),
        ]}
        renderOrder={99}
      >
        <planeGeometry
          args={[VOXEL_CONFIG.SIZE * 0.95, VOXEL_CONFIG.SIZE * 0.95]}
        />
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
