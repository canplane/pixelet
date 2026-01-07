import { useRef } from 'react';
import { Canvas } from '@react-three/fiber';
import * as THREE from 'three';
// 🎯 [Add] 후처리 효과 가져오기
import { EffectComposer, SMAA } from '@react-three/postprocessing';

import { useMapNavigation } from '@/entities/geo/model/use-map-navigation';
import { THREE_CONFIG } from '@/entities/geo/config/three-config';
import { MAP_LAYOUT_THEMES } from '../../model/world-themes';

import { MapCameraController } from '../camera-controller/map-camera-controller';
import { MapOverlays } from '../overlays/map-overlays';
import { MapEnvironment } from './map-environment';
import { MapToastController } from '../toasts/map-toast-controller';
import { MapHtmlOverlay } from '../portal/map-html-overlay';
import { WorldContent } from './world-content';
import { MapDebugTools } from '../debug/map-debug-tools';

const { FOV, NEAR } = THREE_CONFIG.CAMERA;
const { FAR_CLIP } = THREE_CONFIG.RENDERING;

/**
 * [map-view]
 * 3D 캔버스와 2D 오버레이를 통합 관리하는 최상위 뷰 컴포넌트입니다.
 * 테마 전환 시 CSS 배경색과 3D 공간의 안개(fog) 색상을 동기화하여 시각적 연속성을 보장합니다.
 */
export function MapView() {
  const isDebug = import.meta.env.DEV;

  // 🎯 [World Group Ref]
  // 이 그룹 안에 타일, POI, 그리고 Html 포탈 앵커가 모두 포함됩니다.
  // Floating Origin Shift 발생 시 이 그룹 전체가 이동하므로 이격이 발생하지 않습니다.
  const worldGroupRef = useRef<THREE.Group>(null);

  const { themeKey } = useMapNavigation();
  const currentTheme = MAP_LAYOUT_THEMES[themeKey];

  return (
    <div className="fixed inset-0 overflow-hidden bg-black">
      <MapToastController />

      {/* 테마 배경 레이어: 안개 색상과 일치시켜 3D와 2D 영역의 경계를 제거합니다. */}
      <div
        className="absolute inset-0 transition-colors duration-[1500ms] ease-in-out"
        style={{
          backgroundColor: currentTheme.fog,
        }}
      >
        <Canvas
          dpr={1} // 성능과 선명도를 위해 1로 고정 추천 (또는 [1, 1.5])
          gl={{
            antialias: false,
            powerPreference: 'high-performance',
            alpha: false,
            stencil: false,
            depth: true,
          }}
          shadows
          camera={{
            fov: FOV,
            near: NEAR,
            far: FAR_CLIP,
          }}
          frameloop="always"
        >
          {/* 카메라 제어 및 Floating Origin 연동 */}
          <MapCameraController />

          {/* 3D 환경 구성 (조명 및 하늘) */}
          <MapEnvironment />

          {/* 🏗️ 물리적으로 이동하는 세상 (지형, 건물, 그리드) */}
          {/* 여기에 ref를 연결하여 카메라 컨트롤러가 위치를 밀어버립니다 */}
          <WorldContent ref={worldGroupRef} />

          {/* 🎯 [Visual Quality] 복셀 노이즈 제거를 위한 안티앨리어싱 */}
          {/* multisampling은 0으로 꺼야 후처리가 제대로 먹히는 경우가 많으나, 
              R3F v8에서는 auto로 둬도 됩니다. SMAA가 덮어씁니다. */}
          <EffectComposer multisampling={0}>
            <SMAA />
          </EffectComposer>
        </Canvas>
      </div>

      {/* HTML 요소 기반의 오버레이 포탈 */}
      <MapHtmlOverlay />

      {/* 일반 절대 위치 UI 레이어 */}
      <MapOverlays />

      {/* 개발 환경 전용 타일 데이터 추출 도구 */}
      {isDebug && <MapDebugTools />}
    </div>
  );
}
