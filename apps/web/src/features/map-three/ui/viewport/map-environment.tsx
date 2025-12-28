import { useRef } from 'react';
import * as THREE from 'three';
import { useFrame } from '@react-three/fiber';
import { Sky } from '@react-three/drei';
import { useMapNavigation } from '../../model/use-map-navigation';
import { MAP_LAYOUT_THEMES } from '../../model/world-themes';
import { THREE_CONFIG } from '../../model/three-config';

const { FOG_DENSITY } = THREE_CONFIG.RENDERING;

/**
 * [map-environment]
 * 맵의 전반적인 대기 환경(하늘, 안개, 조명)을 통합 관리하는 컴포넌트입니다.
 * 테마 전환 시 선형 보간(lerp)을 통해 시각적 상태를 부드럽게 변경합니다.
 */
export function MapEnvironment() {
  const { themeKey } = useMapNavigation();
  const targetTheme = MAP_LAYOUT_THEMES[themeKey];

  // 유니폼 및 라이트 제어를 위한 Refs
  const skyRef = useRef<any>(null);
  const fogColor = useRef(new THREE.Color(targetTheme.fog));
  const ambientRef = useRef<THREE.AmbientLight>(null);
  const sunRef = useRef<THREE.DirectionalLight>(null);

  // 🎯 [Axis-Mapping] 테마의 [x, y, z] 데이터를 [x, z, -y] 형태로 변환하여 추적
  // 테마 데이터 구조가 { x: East, y: North, z: Altitude }라고 가정합니다.
  const getMappedPos = (pos: [number, number, number]) =>
    new THREE.Vector3(pos[0], pos[2], -pos[1]);

  const sunPosVec = useRef(getMappedPos(targetTheme.sky.sunPosition));

  /**
   * [update-environment-lerp]
   * 매 프레임마다 현재 상태를 타겟 테마의 값으로 보간하여 부드러운 시간 변화를 표현합니다.
   */
  useFrame((_, delta) => {
    const lerpFactor = delta * 1.5;
    const targetSunPos = getMappedPos(targetTheme.sky.sunPosition);

    // --- 1. 안개(Fog) 보간 ---
    fogColor.current.lerp(new THREE.Color(targetTheme.fog), lerpFactor);

    // --- 2. 하늘(Sky) 쉐이더 유니폼 보간 ---
    if (skyRef.current) {
      const { uniforms } = skyRef.current.material;
      const targetPos = new THREE.Vector3(...targetTheme.sky.sunPosition);

      // 🚀 [수정]: targetTheme.sky.sunPosition 대신 targetSunPos를 사용하여 정합성 유지
      sunPosVec.current.lerp(targetSunPos, lerpFactor);
      uniforms.sunPosition.value.copy(sunPosVec.current);

      uniforms.rayleigh.value = THREE.MathUtils.lerp(
        uniforms.rayleigh.value,
        targetTheme.sky.rayleigh,
        lerpFactor,
      );

      uniforms.mieCoefficient.value = THREE.MathUtils.lerp(
        uniforms.mieCoefficient.value,
        targetTheme.sky.mieCoefficient,
        lerpFactor,
      );
    }

    // --- 3. 환경광(Ambient Light) 보간 ---
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

    // --- 4. 직사광(Directional Light) 보간 ---
    if (sunRef.current) {
      // 🎯 이제 uniforms와 동일한 sunPosVec을 사용하므로 광원과 해의 위치가 일치합니다.
      sunRef.current.position.copy(sunPosVec.current);
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
      {/* 물리 기반 하늘 렌더링 */}
      <Sky
        ref={skyRef}
        distance={450000}
        // 초기 포지션도 스와핑된 벡터 사용
        sunPosition={sunPosVec.current as any}
        rayleigh={targetTheme.sky.rayleigh}
        mieCoefficient={targetTheme.sky.mieCoefficient}
      />

      {/* 대기 안개 효과 */}
      <fogExp2 attach="fog" args={[fogColor.current, FOG_DENSITY]} />

      {/* 기본 환경 조명 */}
      <ambientLight ref={ambientRef} />

      {/* 태양광 및 그림자 설정 */}
      <directionalLight
        ref={sunRef}
        castShadow
        shadow-mapSize={[2048, 2048]}
        // 그림자 범위가 광범위하므로 Floating Origin과 연동되어야 함
        shadow-camera-left={-2500}
        shadow-camera-right={2500}
        shadow-camera-top={2500}
        shadow-camera-bottom={-2500}
        shadow-camera-far={5000}
      />

      {/* 반사광 표현을 위한 반구 조명 */}
      <hemisphereLight
        args={[
          targetTheme.lighting.hemisphere.sky,
          targetTheme.lighting.hemisphere.ground,
          targetTheme.lighting.hemisphere.intensity,
        ]}
      />
    </>
  );
}
