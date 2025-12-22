// 1. 외부 라이브러리 (External Libraries)
import { useRef } from 'react';
import { Canvas } from '@react-three/fiber';
import * as THREE from 'three';

// 4. 현재 기능 전용 모델 및 설정 (Feature: map-three)
import { useMapNavigation } from '../../model/use-map-navigation';
import { THREE_CONFIG } from '../../model/three-config';

// 5. 현재 기능 내부 컴포넌트 및 오버레이 (Internal UI)
import { MapCameraController } from '../controls/map-camera-controller';

import { MapOverlays } from '../overlays/map-overlays';
import { MapEnvironment } from './map-environment';
import { MapToastController } from '../toasts/map-toast-controller';
import { MapPortalRoot } from '../portal/map-portal-root';
import { MAP_THEMES } from '../../model/map-theme';
import { WorldContent } from './world-content';

const { FOV, NEAR } = THREE_CONFIG.CAMERA;
const { FAR_CLIP } = THREE_CONFIG.RENDERING;

/** 메인 지도 컴포넌트 */
export function MapView() {
  const worldGroupRef = useRef<THREE.Group>(null);

  const { themeKey } = useMapNavigation();

  const currentTheme = MAP_THEMES[themeKey];

  return (
    <div className="fixed inset-0">
      <MapToastController />

      <div
        className="absolute inset-0 transition-colors duration-[2000ms] ease-in-out"
        style={{
          backgroundColor: currentTheme.background,
        }}
      >
        <Canvas
          // 🚀 [보정] dpr을 [1, 2]로 유지하되, 고해상도 디스플레이에서 성능을 위해 조정
          dpr={window.devicePixelRatio > 2 ? [1, 2] : 1}
          shadows // 그림자가 필요하다면 활성화
          camera={{
            fov: FOV,
            near: NEAR,
            far: FAR_CLIP,
          }}
          // 🚀 [최적화] 프레임 루프를 조금 더 효율적으로 관리
          gl={{
            antialias: true,
            powerPreference: 'high-performance',
            alpha: false, // 배경색을 CSS로 처리하므로 false가 성능에 유리
          }}
        >
          {/* 3D 환경 구성 (조명, 안개, 그리드) */}
          {/* 환경 요소는 그룹 밖에 두어 카메라와 동기화되게 합니다 */}
          <MapEnvironment />

          {/* 카메라 컨트롤 */}
          <MapCameraController worldGroupRef={worldGroupRef} />

          {/* 분리된 월드 그룹 컴포넌트 */}
          <WorldContent ref={worldGroupRef} />
        </Canvas>
      </div>

      {/* 캔버스 바로 뒤에 배치하여 동일한 좌표계(inset-0) 공유 */}
      <MapPortalRoot />

      {/* Overlay Layer: Canvas 위에 띄움 */}
      <MapOverlays />
    </div>
  );
}
