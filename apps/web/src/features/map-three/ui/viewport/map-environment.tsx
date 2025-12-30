import { useRef } from 'react';
import * as THREE from 'three';
import { useFrame, useThree } from '@react-three/fiber';
import { Sky } from '@react-three/drei';
import { useMapNavigation } from '@/entities/osm/model/use-map-navigation';
import { MAP_LAYOUT_THEMES } from '../../model/world-themes';
import { THREE_CONFIG } from '@/entities/osm/config/three-config';

// 🎯 [config-integration] 렌더링 설정 가져오기
const { FOG_DENSITY, FAR_CLIP } = THREE_CONFIG.RENDERING;

// 🎯 [shadow-policy] 가시거리에 맞춰 그림자 카메라도 확장합니다.
// 너무 넓으면 그림자 해상도가 떨어지므로, 적절한 타협점(가시거리의 80% 수준)을 찾습니다.
const SHADOW_SIZE = FAR_CLIP * 0.8;

/**
 * [map-environment]
 * 맵의 전반적인 대기 환경(하늘, 안개, 조명)을 통합 관리하는 컴포넌트입니다.
 * 테마 전환 시 선형 보간(lerp)을 통해 시각적 상태를 부드럽게 변경합니다.
 */
export function MapEnvironment() {
  const { scene } = useThree(); // 🎯 실제 Scene 객체 접근 (Fog 제어용)
  const { themeKey } = useMapNavigation();
  const targetTheme = MAP_LAYOUT_THEMES[themeKey];

  // 유니폼 및 라이트 제어를 위한 Refs
  const skyRef = useRef<any>(null);
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
    // 🎯 [Fix] Ref값만 바꾸는 게 아니라 실제 Scene의 Fog 객체를 업데이트해야 함
    if (scene.fog instanceof THREE.FogExp2) {
      scene.fog.color.lerp(new THREE.Color(targetTheme.fog), lerpFactor);
    }

    // --- 2. 하늘(Sky) 쉐이더 유니폼 보간 ---
    if (skyRef.current) {
      const { uniforms } = skyRef.current.material;

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
      {/* 🎯 Fog는 초기값으로 세팅하고, 이후 useFrame에서 scene.fog를 직접 제어 */}
      <fogExp2 attach="fog" args={[targetTheme.fog, FOG_DENSITY]} />

      {/* 기본 환경 조명 */}
      <ambientLight ref={ambientRef} />

      {/* 태양광 및 그림자 설정 */}
      <directionalLight
        ref={sunRef}
        castShadow
        shadow-mapSize={[2048, 2048]}
        // 그림자 범위가 광범위하므로 Floating Origin과 연동되어야 함
        // 🎯 [Dynamic-Frustum] 설정된 SHADOW_SIZE에 맞춰 그림자 범위 자동 조정
        shadow-camera-left={-SHADOW_SIZE}
        shadow-camera-right={SHADOW_SIZE}
        shadow-camera-top={SHADOW_SIZE}
        shadow-camera-bottom={-SHADOW_SIZE}
        shadow-camera-far={FAR_CLIP * 1.2}
        // 🎯 [Shadow-Bias] 거대한 씬에서의 그림자 깨짐(Artifact) 방지
        shadow-bias={-0.0001}
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
