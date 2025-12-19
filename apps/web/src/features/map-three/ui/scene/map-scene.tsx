// 1. 외부 라이브러리 (External Libraries)
import { useState } from 'react';
import { Canvas } from '@react-three/fiber';

// 2. 프로젝트 공통 타입 및 유틸 (Shared)
import { projection } from '@/shared/lib/geo/projection';

// 4. 현재 기능 전용 모델 및 설정 (Feature: map-three)
import { useMapStore } from '../../model/map-store';
import { THREE_CONFIG } from '../../model/three-config';

// 5. 현재 기능 내부 컴포넌트 및 오버레이 (Internal UI)
import { MapCameraController } from '../controls/map-camera-controller';
import { UserMarker } from '../markers/user-marker';
import { CellHighlighter } from '../markers/cell-highlighter';

import { MapOverlays } from '../overlays/map-overlays';
import { OsmLayer } from '../osm/osm-layer';
import { MapEnvironment } from './map-environment';

/** 메인 지도 컴포넌트 */
export function MapScene() {
  // Store에서 필요한 상태와 액션 구독
  const center = useMapStore((state) => state.center);

  const [showBuildings, setShowBuildings] = useState(false);

  return (
    <div className="relative w-screen h-screen bg-[#111] overflow-hidden">
      {/* Overlay Layer: Canvas 위에 띄움 */}
      <MapOverlays />

      <div className="absolute top-24 left-5 z-50 bg-black/50 p-2 rounded text-white text-sm">
        <label className="flex items-center gap-2 cursor-pointer">
          <input
            type="checkbox"
            checked={showBuildings}
            onChange={(e) => setShowBuildings(e.target.checked)}
          />
          건물 표시 (OSM)
        </label>
      </div>

      <Canvas
        dpr={[1, 2]} // 성능 모드 활성화 (안티앨리어싱 등 조절)
        camera={{
          fov: THREE_CONFIG.CAMERA.FOV,
          // 초기 위치는 의미가 없습니다. 컨트롤러가 center로 즉시 옮깁니다.
          // position: [initialPos.x, initialPos.y, initialPos.z],
          near: THREE_CONFIG.CAMERA.NEAR,
          far: THREE_CONFIG.RENDERING.FAR_CLIP, // FAR 값을 충분히 크게 설정하세요 (예: 5000)
        }}
      >
        {/* 3D 환경 구성 (조명, 안개, 그리드) */}
        <MapEnvironment />

        {/* 카메라 컨트롤 */}
        <MapCameraController />

        {/* 내 위치 마커 */}
        <UserMarker />

        {/* 3. OSM 데이터 전용 레이어: 내부적으로 fetch와 렌더링 수행 */}
        <OsmLayer showBuildings={showBuildings} />

        {/* 테스트용 빨간 사각형 (실제 100m x 100m) */}
        {center && (
          <mesh
            rotation={[-Math.PI / 2, 0, 0]}
            position={[center.x, center.y, center.z]}
          >
            <planeGeometry args={[100, 100]} />
            <meshBasicMaterial color="red" wireframe />
          </mesh>
        )}

        <CellHighlighter />
      </Canvas>
    </div>
  );
}
