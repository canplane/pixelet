import { useRef } from 'react';
import * as THREE from 'three';
import { useFrame } from '@react-three/fiber'; // 🚀 애니메이션을 위해 추가
import { Sky, Grid } from '@react-three/drei';
import { useMapStore } from '../../model/map-store';
import { projection } from '@/shared/lib/geo/projection';
import { MAP_THEMES } from '../../model/map-theme';
import { THREE_CONFIG } from '../../model/three-config';

const { GRID_SIZE } = projection;
const { FOG_NEAR, FOG_FAR } = THREE_CONFIG.RENDERING;

export function MapEnvironment() {
  // 🚀 [추가] 오프셋 구독 (그리드 정렬용)
  const originOffset = useMapStore((state) => state.originOffset);

  // 🚀 현재 시간을 기반으로 테마 결정 (store에 저장하거나 로직 추가 가능)
  const themeKey = useMapStore((state) => state.getThemeKey());
  const targetTheme = MAP_THEMES[themeKey];

  // 보간을 위한 내부 Ref들
  const fogColor = useRef(new THREE.Color(targetTheme.fog));
  const ambientRef = useRef<THREE.AmbientLight>(null);
  const sunRef = useRef<THREE.DirectionalLight>(null);
  // 🚀 조명 위치 보간을 위한 Vector3 추가
  const sunPosVec = useRef(new THREE.Vector3(...targetTheme.sky.sunPosition));

  useFrame((_, delta) => {
    const lerpFactor = delta * 1.5;

    // 1. 색상 및 강도 보간 (기존 로직)
    fogColor.current.lerp(new THREE.Color(targetTheme.fog), lerpFactor);

    if (ambientRef.current) {
      ambientRef.current.color.lerp(
        new THREE.Color(targetTheme.lighting.ambient.color),
        lerpFactor,
      );
      ambientRef.current.intensity = THREE.MathUtils.lerp(
        ambientRef.current.intensity,
        targetTheme.lighting.ambient.intensity,
        lerpFactor,
      );
    }

    if (sunRef.current) {
      // 🚀 2. 조명의 위치를 테마의 sunPosition으로 부드럽게 이동
      const targetPos = targetTheme.sky.sunPosition;
      sunPosVec.current.lerp(new THREE.Vector3(...targetPos), lerpFactor);
      sunRef.current.position.copy(sunPosVec.current);

      // 🚀 3. 조명 색상 및 강도 보간
      sunRef.current.color.lerp(
        new THREE.Color(targetTheme.lighting.directional.color),
        lerpFactor,
      );
      sunRef.current.intensity = THREE.MathUtils.lerp(
        sunRef.current.intensity,
        targetTheme.lighting.directional.intensity,
        lerpFactor,
      );
    }
  });

  return (
    <>
      {/* 1. 동화 속 하늘: 레일리 산란을 높여 푸르고 몽환적인 대기 형성 */}
      {/* Sky는 쉐이더 내부 연산이므로 target을 직접 주되, 
          sunPosition 등은 useFrame에서 직접 조정하면 더 정교해집니다. */}
      <Sky
        distance={450000}
        // 🚀 Sky의 태양 위치도 보간된 값을 쓰면 더 부드럽지만,
        // 일단 target을 주어 하늘 색감을 잡습니다.
        sunPosition={targetTheme.sky.sunPosition}
        rayleigh={targetTheme.sky.rayleigh}
        mieCoefficient={targetTheme.sky.mieCoefficient}
      />
      {/* 2. 따뜻한 안개: 약간 어둡거나 푸른 감이 도는 회색 톤 */}
      <fog
        attach="fog"
        args={[
          fogColor.current.getStyle(),
          FOG_NEAR, // 600
          FOG_FAR, // 1200
        ]}
      />

      {/* 3. 조명 시스템: 화사하고 입체적인 조명 구성 */}
      <ambientLight ref={ambientRef} />
      {/* 🚀 position을 초기값으로 설정하되, useFrame에서 실시간으로 갱신됩니다. */}
      <directionalLight
        ref={sunRef}
        position={targetTheme.sky.sunPosition}
        castShadow
        shadow-camera-far={5000}
      />
      {/* 반사광 추가: 지면에서 올라오는 부드러운 빛 */}
      <hemisphereLight
        args={[
          targetTheme.lighting.hemisphere.sky,
          targetTheme.lighting.hemisphere.ground,
          targetTheme.lighting.hemisphere.intensity,
        ]}
      />
      {/* 초록색 배경 평면 (디버그) */}
      <mesh rotation={[-Math.PI / 2, 0, 0]} position={[0, -0.05, 0]}>
        <planeGeometry args={[10000, 10000]} />
        <meshBasicMaterial color="#4c9540" />
      </mesh>
    </>
  );
}
