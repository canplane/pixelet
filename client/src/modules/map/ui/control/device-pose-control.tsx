// @pxl/web/modules/map/ui/control/device-pose-control.tsx

import { Compass, Loader2, Locate, Navigation } from "lucide-react";

import {
  type DeviceLocation,
  type DeviceOrientation,
  DevicePoseStatus,
  useDeviceLocationStore,
  useDeviceOrientationStore,
} from "@/modules/device/pose/store.ts";
import { useDeviceOrientation } from "@/modules/device/pose/use-device-orientation.ts";
import { useMapStore } from "@/modules/map/store.ts";
import { useMapNavigation } from "@/modules/map/navigation/use-map-navigation.ts";
import { useCameraStore } from "@/modules/camera/store.ts";
import { useFollowMode } from "@/modules/map/navigation/use-follow-mode.ts";

import { cn } from "@/shared/lib/utils/cn.ts";
import { CircularButton } from "@/shared/ui/theme/cozy/circular-button.tsx";

import type { f64 } from "@pixelet/shared/kit/word.ts";

export function DevicePoseControl() {
  useFollowMode();

  const location_status: DevicePoseStatus = useDeviceLocationStore((state) => state.status);
  const location: DeviceLocation | null = useDeviceLocationStore((state) => state.location);

  const orientation: DeviceOrientation | null = useDeviceOrientationStore((state) => state.orientation);
  const { request_orientation } = useDeviceOrientation();

  const set_map_center = useMapStore.getState().set_map_center;

  const view_state = useCameraStore((state) => state.view_state);
  const is_follow_mode = useCameraStore((state) => state.is_follow_mode);
  const set_is_follow_mode = useCameraStore.getState().set_is_follow_mode;
  const reset_orientation = useCameraStore.getState().reset_orientation;

  const { device_coord, is_off_center } = useMapNavigation();

  // Show "move to my location" only when off-center and not following.
  const can_show_location_control: boolean = is_off_center && !is_follow_mode;

  const { azimuth } = view_state;

  const is_enabled: boolean = location_status === DevicePoseStatus.READY && !!location;
  const is_loading: boolean = location_status === DevicePoseStatus.LOADING;

  const handle_action = async () => {
    if (!is_enabled || is_loading) return;

    // 1️⃣ 센터 이탈 상태라면? -> 내 위치로 즉시 복귀
    if (can_show_location_control) {
      if (device_coord) {
        set_map_center(device_coord);
      }
      set_is_follow_mode(false);
      return;
    }

    // 2️⃣ 센터에 있다면? -> 팔로우 모드(나침반 모드) 토글
    if (!is_follow_mode) {
      // 1. 권한 요청 및 리스너 가동
      const success: boolean = await request_orientation();
      if (success) {
        set_is_follow_mode(true);
      } else {
        // 권한 거부 시 토스트 메시지 등을 띄워주면 좋습니다.
        console.error("Orientation 센서를 가동할 수 없습니다.");
      }
    } else {
      // 팔로우 모드 해제 시 북쪽 정렬
      set_is_follow_mode(false);
      reset_orientation();
    }
  };

  const get_icon_style = () => {
    if (is_loading) {
      return { Icon: Loader2, className: "animate-spin opacity-40" };
    }

    if (can_show_location_control) {
      return { Icon: Locate, className: "text-[#5B4636]/40 scale-90" };
    }

    if (is_follow_mode) {
      const heading_deg: f64 = orientation?.heading ?? 0;
      const final_rotation_deg: f64 = heading_deg - (azimuth * 180) / Math.PI;

      return {
        Icon: Compass,
        style: { transform: `rotate(${final_rotation_deg}deg)` },
        className: "text-sky-500 scale-110",
      };
    }

    return {
      Icon: Navigation,
      className: "text-[#5B4636] group-hover:rotate-[45deg]",
    };
  };

  const { Icon, className, style } = get_icon_style();

  return (
    <CircularButton
      className="w-11 h-11"
      onClick={handle_action}
      isActive={is_follow_mode}
      disabled={!is_enabled || is_loading}
    >
      <Icon
        className={cn(
          "w-5 h-5 transition-all duration-500 ease-out",
          className,
        )}
        style={style}
      />
    </CircularButton>
  );
}
