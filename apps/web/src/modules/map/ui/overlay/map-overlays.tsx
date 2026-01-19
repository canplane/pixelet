// @pxl/web/modules/map/ui/overlay/map-overlays.tsx

import { cn } from "@/shared/lib/utils/cn.ts";
import { MapCompass } from "../control/compass.tsx";
import { MapScaleBar } from "../control/scale-bar.tsx";
// import { WorldInspector } from "./world-inspector.tsx";
import { DevicePoseControl } from "@/modules/map/ui/control/device-pose-control.tsx";
import { useMapStore } from "@/modules/map/navigation/map-store.ts";
import { CameraControlMode } from "@/modules/world/render/config.ts";
// import { PoiDetailPanel } from "./poi-detail-panel.tsx";
// import { MapSearchBar } from "./map-search-bar.tsx";
// import { Badge } from "@/shared/ui/theme/glass/badge.tsx";

/**
 * 3D 캔버스 위에 렌더링되는 모든 2D UI 컴포넌트들을 통합 관리합니다.
 * 지도 조작 상태(is_interacting)에 따라 UI의 시각적 피드백(Blur, Scale, Opacity)을 동적으로 제어합니다.
 */
export function MapOverlays() {
  const camera_control_mode = useMapStore((state) => state.camera_control_mode);
  const is_interacting: boolean = camera_control_mode !== CameraControlMode.NONE;

  /**
   * [get-interaction-style]
   * 사용자가 지도를 조작 중일 때 UI가 시야를 방해하지 않도록 감쇠 효과를 적용합니다.
   * 조작 중에는 pointer-events-none을 통해 클릭 간섭을 차단합니다.
   */
  const interactionStyle = cn(
    "transition-all duration-500 ease-in-out",
    is_interacting ? "pointer-events-none opacity-40 scale-[0.98] blur-[0.5px]" : "opacity-100 scale-100 blur-0",
  );

  return (
    <>
      {/* 1. 상단 중앙: 위치 검색바 */}
      {
        /* <div className="absolute top-8 left-1/2 -translate-x-1/2 z-100 w-[calc(100%-4rem)] flex justify-center pointer-events-none">
        <MapSearchBar className={cn(interactionStyle, "pointer-events-auto")} />
      </div> */
      }

      {/* 2. 상단 좌측: Voxel 데이터 모니터링 패널 */}
      <div
        className={cn(
          "absolute top-8 left-8 z-50 w-64 pointer-events-none",
          interactionStyle,
        )}
      >
        {
          /* <div className="pointer-events-auto">
          <WorldInspector />
        </div> */
        }
      </div>

      {/* 3. 상단 우측: 선택된 POI 정보 패널 */}
      <div
        className={cn(
          "absolute top-8 right-8 z-50 w-64 pointer-events-none",
          interactionStyle,
        )}
      >
        {
          /* <div className="pointer-events-auto">
          <PoiDetailPanel />
        </div> */
        }
      </div>

      {/* 4. 하단 좌측: 원점 이동 디버그 유틸리티 */}
      <div className={cn("absolute bottom-8 left-8 z-50", interactionStyle)} />

      {/* 5. 하단 우측: 지도 제어 도구 (나침반, 스케일바, 위치 컨트롤) */}
      <div
        className={cn(
          "absolute bottom-8 right-8 z-50 flex flex-col items-end gap-5 pointer-events-none",
          interactionStyle,
        )}
      >
        <div className={cn(!is_interacting && "pointer-events-auto")}>
          <MapCompass />
        </div>

        <div
          className={cn(
            "flex items-end gap-6",
            !is_interacting && "pointer-events-auto",
          )}
        >
          <div className="pb-4">
            <MapScaleBar />
          </div>
          <DevicePoseControl />
        </div>
      </div>
    </>
  );
}
