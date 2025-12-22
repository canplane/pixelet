import { useRef } from 'react';
import * as THREE from 'three';
import { useFrame } from '@react-three/fiber'; // 🚀 애니메이션을 위해 추가
import { Sky } from '@react-three/drei';
import { useMapNavigation } from '../../model/use-map-navigation';
import { MAP_THEMES } from '../../model/map-theme';
import { THREE_CONFIG } from '../../model/three-config';

const { FOG_NEAR, FOG_FAR } = THREE_CONFIG.RENDERING;

export function MapEnvironment() {
  const { themeKey } = useMapNavigation();
  const isNight = themeKey === 'night';

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

      {/* 🚀 초록색 평면을 세련된 디지털 캔버스로 교체 */}
      <mesh
        rotation={[-Math.PI / 2, 0, 0]}
        position={[0, -0.1, 0]}
        receiveShadow
      >
        <planeGeometry args={[10000, 10000]} />
        <meshStandardMaterial
          // 🚀 밤에는 깊은 네이비/블랙, 낮에는 아주 밝은 그레이/오프화이트
          color={isNight ? '#05070a' : '#f0f2f5'}
          roughness={1}
          metalness={0}
        />
      </mesh>

      {/* 🚀 (옵션) 무한 격자 보조도구: 지면이 너무 밋밋할 때 '눈금'을 줍니다. */}
      {/* <gridHelper
        args={[
          10000,
          500,
          isNight ? '#1e293b' : '#e2e8f0',
          isNight ? '#0f172a' : '#f1f5f9',
        ]}
        position={[0, -0.09, 0]}
      /> */}
    </>
  );
}
