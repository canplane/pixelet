// @pxl/web/modules/map/scene.tsx

import { useRef } from "react";
import { Canvas } from "@react-three/fiber";
import * as THREE from "three";
import { EffectComposer, SMAA } from "@react-three/postprocessing";

import { render_cfg } from "@/modules/world/render/config.ts";
import { get_theme_key, themes, type WorldTheme } from "@/modules/world/render/themes.ts";
import { CameraController } from "@/modules/world/camera/camera-controller.tsx";

import { MapEnvironment } from "./environment.tsx";
import { World } from "@/modules/world/world.tsx";
import { MapOverlays } from "./ui/overlay/map-overlays.tsx";
import { MapHTMLPortal } from "./portal/html-portal.tsx";
import { WorldExportTools } from "@/modules/world/terrain/ui/export-tools.tsx";

import { MapToastController } from "./ui/toast/toast-controller.tsx";

const { FOV, NEAR } = render_cfg.camera;
const { FAR_CLIP } = render_cfg.rendering;

export function MapScene() {
  const world_ref = useRef<THREE.Group>(null);

  const theme_key = get_theme_key();
  const theme: WorldTheme = themes[theme_key];

  return (
    <div className="fixed inset-0 overflow-hidden bg-black">
      <MapToastController />

      {/* 테마 배경 레이어: 안개 색상과 일치시켜 3D와 2D 영역의 경계를 제거합니다. */}
      <div
        className="absolute inset-0 transition-colors duration-1500 ease-in-out"
        style={{
          backgroundColor: theme.fog,
        }}
      >
        <Canvas
          dpr={1} // 성능과 선명도를 위해 1로 고정 추천 (또는 [1, 1.5])
          gl={{
            antialias: false,
            powerPreference: "high-performance",
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
          <CameraController />

          {/* 3D 환경 구성 (조명 및 하늘) */}
          <MapEnvironment />

          {/* 🏗️ 물리적으로 이동하는 세상 (지형, 건물, 그리드) */}
          {/* 여기에 ref를 연결하여 카메라 컨트롤러가 위치를 밀어버립니다 */}
          <World ref={world_ref} />

          {/* 🎯 [Visual Quality] 복셀 노이즈 제거를 위한 안티앨리어싱 */}
          {
            /* multisampling은 0으로 꺼야 후처리가 제대로 먹히는 경우가 많으나,
              R3F v8에서는 auto로 둬도 됩니다. SMAA가 덮어씁니다. */
          }
          <EffectComposer multisampling={0}>
            <SMAA />
          </EffectComposer>
        </Canvas>
      </div>

      {/* HTML 요소 기반의 오버레이 포탈 */}
      <MapHTMLPortal />

      {/* 일반 절대 위치 UI 레이어 */}
      <MapOverlays />

      {/* 개발 환경 전용 페이지 데이터 추출 도구 */}
      {__IS_DEV__ && <WorldExportTools />}
    </div>
  );
}
