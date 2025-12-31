import { useRef, useMemo } from 'react';
import * as THREE from 'three';
import type { Vector3Tuple } from 'three';
import { useFrame, useThree } from '@react-three/fiber';
import { Sky } from '@react-three/drei';
import { useMapNavigation } from '@/entities/osm/model/use-map-navigation';
import { MAP_LAYOUT_THEMES } from '../../model/world-themes';
import { THREE_CONFIG } from '@/entities/osm/config/three-config';

// 🎯 [utility-import] 좌표 변환 표준 유틸리티 적용
import { toThreeCoord } from '@/shared/lib/three/three-coords';

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

  /**
   * 🎯 [Coordinate-System-Mapping]
   * Theme Data: Vector3Tuple([East, North, Up]) -> Three.js: Vector3Tuple([x, z, -y])
   * Vector3Tuple을 해체하여 Vector3 인스턴스를 생성합니다.
   */
  const getMappedPos = (pos: Vector3Tuple): THREE.Vector3 => {
    const [x, y, z] = pos;
    // 🚀 [toThreeCoord] 유틸리티는 Vector3Tuple을 반환합니다.
    const threeCoord = toThreeCoord({ x, y, z });
    return new THREE.Vector3(...threeCoord);
  };

  /**
   * 🎯 [Optimization: Memoization]
   * 매 프레임(60fps)마다 새로운 Vector3 인스턴스를 생성하는 것을 방지합니다.
   * 타겟 위치(targetSunPos)는 테마가 바뀔 때만 재계산되면 충분합니다.
   */
  const targetSunPos = useMemo(
    () => getMappedPos(targetTheme.sky.sunPosition),
    // eslint-disable-next-line react-hooks/exhaustive-deps
    [targetTheme], // getMappedPos는 내부 함수지만 로직이 고정적이므로 theme 변경만 감지하면 됨
  );

  // 현재 보간 중인 태양 위치 상태 (Mutable)
  // 초기값은 타겟 위치를 복제(clone)하여 시작 (참조 오염 방지)
  const sunPosVec = useRef(targetSunPos.clone());

  /**
   * [update-environment-lerp]
   * 매 프레임마다 현재 상태를 타겟 테마의 값으로 보간하여 부드러운 시간 변화를 표현합니다.
   */
  useFrame((_, delta) => {
    // 🎯 [Safety-Guard] 초기화 중에는 실행 스킵
    if (!scene || !skyRef.current) return;

    const lerpFactor = delta * 1.5;

    // --- 1. 안개(Fog) 보간 ---
    // FogExp2 타입인지 확인 후 색상 변경
    if (scene.fog && 'color' in scene.fog) {
      (scene.fog as THREE.FogExp2).color.lerp(
        new THREE.Color(targetTheme.fog),
        lerpFactor,
      );
    }

    // --- 2. 하늘(Sky) 보간 ---
    if (skyRef.current?.material?.uniforms) {
      const { uniforms } = skyRef.current.material;

      // 🎯 [Optimization] 미리 계산된 targetSunPos를 사용하여 메모리 할당 최소화
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
      {/* 🎯 Fog는 Scene에 자동 attach되므로 여기서 선언하면 scene.fog가 됩니다 */}
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
        // 🎯 [Shadow-Artifact-Fix] 곡면이나 경사면에서 그림자가 깨지는 것을 방지
        shadow-normalBias={0.05}
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
