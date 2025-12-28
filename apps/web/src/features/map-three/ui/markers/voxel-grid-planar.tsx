import { useMemo } from 'react';
import * as THREE from 'three';
import { useMapNavigation } from '../../model/use-map-navigation';
import { MAP_LAYOUT_THEMES } from '../../model/world-themes';
import { VOXEL_CONFIG } from '@pixelet/shared/entities/voxel/voxel-config';
import { THREE_CONFIG } from '../../model/three-config';
import { toThreePosWithOffset } from '@/shared/lib/three/three-coords';

/**
 * [voxel-grid-planar]
 * 지형 레이어 위에 안착되는 절차적 쉐이더 그리드입니다.
 * 물리적인 높이는 바닥(0)에 고정하되, 렌더링 우선순위를 조절하여 지형 위에 덧씌웁니다.
 */
export const VoxelGridPlanar = () => {
  const { SIZE } = VOXEL_CONFIG;
  const { FADE_END } = THREE_CONFIG.RENDERING;
  const { themeKey } = useMapNavigation();
  const targetTheme = MAP_LAYOUT_THEMES[themeKey];

  const material = useMemo(() => {
    return new THREE.ShaderMaterial({
      transparent: true,
      // [rendering-priority]
      // 그리드 뒤의 오브젝트 판정에 영향을 주지 않도록 depth-write를 끕니다.
      depthWrite: false,
      depthTest: true,
      // [z-fighting-prevention]
      // 동일 좌표(y:0)에서 지형보다 항상 위에 그려지도록 폴리곤 오프셋을 설정합니다.
      polygonOffset: true,
      polygonOffsetFactor: -4,
      polygonOffsetUnits: -4,
      uniforms: {
        uColor: { value: new THREE.Color(targetTheme.grid.cell) },
        uSize: { value: SIZE },
        uFadeEnd: { value: FADE_END },
      },
      vertexShader: `
        varying vec3 vWorldPosition;
        void main() {
          vec4 worldPosition = modelMatrix * vec4(position, 1.0);
          vWorldPosition = worldPosition.xyz;
          gl_Position = projectionMatrix * viewMatrix * worldPosition;
        }
      `,
      fragmentShader: `
        varying vec3 vWorldPosition;
        uniform vec3 uColor;
        uniform float uSize;
        uniform float uFadeEnd;

        float getRefinedGrid(float position, float size, float softness) {
          float dist = abs(fract(position / size - 0.5) - 0.5) * size;
          // [softness-tuning] 0.04 수치로 보들보들한 경계면을 형성합니다.
          return smoothstep(softness, 0.0, dist);
        }

        void main() {
          // 🎯 [Coordinate-Fix]: 지표면 평면은 X와 Z입니다. (Y는 고도)
          float flatDist = length(vWorldPosition.xz); 
          float softness = 0.04;
          
          // 🎯 World X축 격자와 World Z축(지리적 북향) 격자를 각각 생성
          float lineX = getRefinedGrid(vWorldPosition.x, uSize, softness);
          float lineZ = getRefinedGrid(vWorldPosition.z, uSize, softness); 
          float lineAlpha = max(lineX, lineZ);
          
          // 🎯 평면 거리(flatDist)를 기준으로 페이드 처리
          float fade = pow(clamp(1.0 - flatDist / uFadeEnd, 0.0, 1.0), 3.0);
          // [opacity-control] 0.10 농도로 연하고 세련된 느낌을 유지합니다.
          float finalAlpha = lineAlpha * fade * 0.10; 
          
          if (finalAlpha < 0.001) discard;
          gl_FragColor = vec4(uColor, finalAlpha);
        }
      `,
    });
  }, [targetTheme, SIZE, FADE_END]);

  return (
    <mesh
      // [layer-order] 지형(0)보다 나중에 그려지도록 설정합니다.
      renderOrder={1}
      // 🎯 THREE_CONFIG에 정의된 [ -Math.PI / 2, 0, 0 ]를 사용하여 바닥에 눕힘
      rotation={THREE_CONFIG.LAYOUT.FLOOR_ROTATION as unknown as THREE.Euler}
      // 🎯 지면(0)보다 아주 미세하게 위(0.01)로 띄워 Z-Fighting 방지
      position={toThreePosWithOffset(
        { x: 0, y: 0 },
        THREE_CONFIG.LAYOUT.OFFSET.GRID,
      )}
    >
      <planeGeometry args={[FADE_END * 2, FADE_END * 2]} />
      <primitive object={material} attach="material" />
    </mesh>
  );
};
