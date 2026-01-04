import { useRef, useMemo } from 'react';
import * as THREE from 'three';
import type { Vector3Tuple } from 'three';
import { useFrame, useThree } from '@react-three/fiber';
import { Sky } from '@react-three/drei';
import { useMapNavigation } from '@/entities/geo/model/use-map-navigation';
import { MAP_LAYOUT_THEMES } from '../../model/world-themes';
import { THREE_CONFIG } from '@/entities/geo/config/three-config';
import { toThreeCoord } from '@/shared/lib/three/three-coords';

// 🎯 [Config] 선형 안개 설정 가져오기
const { FOG_NEAR, FOG_FAR } = THREE_CONFIG.RENDERING;

// 🎯 [Shadow Fix] 그림자 범위를 플레이어 주변 1km(1024m)로 제한
// 기존 10km 범위는 픽셀 해상도가 너무 낮아 지형에 빗금 노이즈를 만듭니다.
const SHADOW_SIZE = 1024;

export function MapEnvironment() {
  const { scene } = useThree();
  const { themeKey } = useMapNavigation();
  const targetTheme = MAP_LAYOUT_THEMES[themeKey];

  const skyRef = useRef<any>(null);
  const ambientRef = useRef<THREE.AmbientLight>(null);
  const sunRef = useRef<THREE.DirectionalLight>(null);

  const getMappedPos = (pos: Vector3Tuple): THREE.Vector3 => {
    const [x, y, z] = pos;
    const threeCoord = toThreeCoord({ x, y, z });
    return new THREE.Vector3(...threeCoord);
  };

  const targetSunPos = useMemo(
    () => getMappedPos(targetTheme.sky.sunPosition),
    [targetTheme],
  );

  const sunPosVec = useRef(targetSunPos.clone());

  useFrame((_, delta) => {
    if (!scene || !skyRef.current) return;
    const lerpFactor = delta * 1.5;

    // --- 1. 안개(Fog) 보간 (Linear Fog) ---
    // Scene에 attach된 fog가 THREE.Fog(선형)인지 확인
    if (scene.fog && 'isFog' in scene.fog) {
      const fog = scene.fog as THREE.Fog;
      // 색상 보간
      fog.color.lerp(new THREE.Color(targetTheme.fog), lerpFactor);

      // 🎯 [Tip] 필요하다면 near/far도 보간할 수 있지만, 보통 거리는 고정합니다.
      // fog.near = THREE.MathUtils.lerp(fog.near, FOG_NEAR, lerpFactor);
      // fog.far = THREE.MathUtils.lerp(fog.far, FOG_FAR, lerpFactor);
    }

    // --- 2. 하늘(Sky) 보간 ---
    if (skyRef.current?.material?.uniforms) {
      const { uniforms } = skyRef.current.material;
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

    // --- 3. 조명 보간 ---
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
      <Sky
        ref={skyRef}
        distance={450000}
        sunPosition={sunPosVec.current as any}
        rayleigh={targetTheme.sky.rayleigh}
        mieCoefficient={targetTheme.sky.mieCoefficient}
      />

      {/* 🎯 [Minecraft Style Fog] 
        fogExp2 대신 일반 fog를 사용합니다.
        attach="fog"를 하면 Three.js Scene에 자동으로 등록되며,
        ShaderMaterial에서 USE_FOG가 정의되어 있으면 자동으로 uniforms(fogNear, fogFar)가 주입됩니다.
      */}
      <fog attach="fog" args={[targetTheme.fog, FOG_NEAR, FOG_FAR]} />

      <ambientLight ref={ambientRef} />

      <directionalLight
        ref={sunRef}
        castShadow
        // 🎯 텍스처 크기를 4096으로 늘리면 더 좋지만, 모바일 성능 고려 2048 유지
        shadow-mapSize={[2048, 2048]}
        // 범위 축소: 2048px / 2048m = 1px당 1m 정밀도 확보
        shadow-camera-left={-SHADOW_SIZE}
        shadow-camera-right={SHADOW_SIZE}
        shadow-camera-top={SHADOW_SIZE}
        shadow-camera-bottom={-SHADOW_SIZE}
        // Far는 적당히 깊게
        shadow-camera-far={4000}
        // 정밀도가 높아졌으므로 바이어스 값을 작게 조정
        shadow-bias={-0.0005}
        shadow-normalBias={0.02}
      />

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
