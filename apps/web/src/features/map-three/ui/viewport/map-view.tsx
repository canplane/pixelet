import { useRef } from 'react';
import { Canvas } from '@react-three/fiber';
import * as THREE from 'three';

import { useMapNavigation } from '@/entities/osm/model/use-map-navigation';
import { THREE_CONFIG } from '@/entities/osm/config/three-config';
import { MAP_LAYOUT_THEMES } from '../../model/world-themes';

import { MapCameraController } from '../controls/map-camera-controller';
import { MapOverlays } from '../overlays/map-overlays';
import { MapEnvironment } from './map-environment';
import { MapToastController } from '../toasts/map-toast-controller';
import { MapPortalRoot } from '../portal/map-portal-root';
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
          dpr={[1, 2]}
          shadows
          camera={{
            fov: FOV,
            near: NEAR,
            far: FAR_CLIP,
          }}
          gl={{
            antialias: true,
            powerPreference: 'high-performance',
            alpha: false,
            stencil: false,
            depth: true,
          }}
          frameloop="always"
        >
          {/* 1. 3D 환경 구성 (조명 및 하늘) */}
          <MapEnvironment />

          {/* 2. 카메라 제어 및 Floating Origin 연동 */}
          <MapCameraController worldGroupRef={worldGroupRef} />

          {/* 3. 월드 콘텐츠 (그리드 지형, 인프라, 마커) */}
          <WorldContent ref={worldGroupRef} />
        </Canvas>
      </div>

      {/* HTML 요소 기반의 오버레이 포탈 및 UI 레이어 */}
      <MapPortalRoot />
      <MapOverlays />

      {/* 개발 환경 전용 그리드 데이터 추출 도구 */}
      {isDebug && <MapDebugTools />}
    </div>
  );
}
