// @pxl/web/modules/map/ui/overlay/map-overlays.tsx

import { cn } from "@/shared/lib/utils/cn.ts";
import { MapCompass } from "../control/compass.tsx";
import { MapScaleBar } from "../control/scale-bar.tsx";
import { DevicePoseControl } from "@/modules/map/ui/control/device-pose-control.tsx";
import { TerrainFollowToggle } from "@/modules/map/ui/control/terrain-follow-toggle.tsx";
import { MotionContainer } from "@/shared/ui/motion/motion-container.tsx";
import { CameraStatus, useCameraStore } from "@/modules/camera/store.ts";

/**
 * 3D 캔버스 위에 렌더링되는 모든 2D UI 컴포넌트들을 통합 관리합니다.
 * 지도 조작 상태(is_interacting)에 따라 UI의 시각적 피드백(Blur, Scale, Opacity)을 동적으로 제어합니다.
 */
export function MapOverlays() {
  const camera_status: CameraStatus = useCameraStore((state) => state.camera_status);
  const is_camera_busy: boolean = camera_status !== CameraStatus.IDLE;

  /**
   * [get-interaction-style]
   * 사용자가 지도를 조작 중일 때 UI가 시야를 방해하지 않도록 감쇠 효과를 적용합니다.
   * 조작 중에는 pointer-events-none을 통해 클릭 간섭을 차단합니다.
   */
  const interaction_style = cn(
    "transition-all duration-500 ease-in-out",
    is_camera_busy ? "pointer-events-none opacity-40 scale-[0.98] blur-[0.5px]" : "opacity-100 scale-100 blur-0",
  );

  return (
    <>
      {/* 1. 상단 중앙: 위치 검색바 */}
      {
        /* <div className="absolute top-8 left-1/2 -translate-x-1/2 z-100 w-[calc(100%-4rem)] flex justify-center pointer-events-none">
        <MapSearchBar className={cn(interaction_style, "pointer-events-auto")} />
      </div> */
      }

      {/* 2. 상단 좌측: Voxel 데이터 모니터링 패널 */}
      <div
        className={cn(
          "absolute top-8 left-8 z-50 w-64 pointer-events-none",
          interaction_style,
        )}
      >
        {
          /* <div className="pointer-events-auto">
          <WorldInspector />
        </div> */
        }
      </div>

      {/* 3. 상단 우측: 선택된 POI 정보 패널 + 지형-따라가기(디버그용, 별도 배치) */}
      <div
        className={cn(
          "absolute top-4 right-4 z-50 flex flex-col items-end gap-2 pointer-events-none",
          interaction_style,
        )}
      >
        <MotionContainer variant="slideUp" delay={0.5}>
          <div className={cn(!is_camera_busy && "pointer-events-auto")}>
            <TerrainFollowToggle />
          </div>
        </MotionContainer>

        <div className="w-64 pointer-events-none">
          {
            /* <div className="pointer-events-auto">
            <PoiDetailPanel />
          </div> */
          }
        </div>
      </div>

      {/* 4. 하단 좌측: 원점 이동 디버그 유틸리티 */}
      <div className={cn("absolute bottom-8 left-8 z-50", interaction_style)} />

      {/* 5. 하단 우측: 지도 제어 도구 (나침반 / 스케일바 - GPS) */}
      <div
        className={cn(
          "absolute bottom-4 right-4 z-50 flex flex-col items-end gap-3 pointer-events-none",
          interaction_style,
        )}
      >
        <div className={cn(!is_camera_busy && "pointer-events-auto")}>
          <MapCompass />
        </div>

        <div className={cn("flex items-center gap-3", !is_camera_busy && "pointer-events-auto")}>
          <MapScaleBar />
          <DevicePoseControl />
        </div>
      </div>
    </>
  );
}
