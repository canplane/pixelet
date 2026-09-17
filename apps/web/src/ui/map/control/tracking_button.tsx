// @pxl/web/ui/map/control/tracking_button.tsx

import { Compass, Loader2, Locate, Navigation } from "lucide-react";
import { useTranslation } from "react-i18next";

import { kernel_tracking, TRACKING_FOLLOW, TRACKING_RECENTER, TRACKING_UNFOLLOW, useKernel } from "@/kernel/kernel.ts";

import { cn } from "@/kit/lib/util/cn.ts";
import { CircularButton } from "@/kit/ui/shadcn/circular_button.tsx";

export function TrackingButton() {
  const { t } = useTranslation();

  const location_status = useKernel((s) => s.location.status);
  const has_location = useKernel((s) => s.location.has_location);
  const heading = useKernel((s) => s.orientation.heading);
  const azimuth = useKernel((s) => s.view.azimuth);
  const is_follow_mode = useKernel((s) => s.is_follow_mode);
  const is_off_center = useKernel((s) => s.map.is_off_center);

  const can_show_location_control: boolean = is_off_center && !is_follow_mode;
  const is_enabled: boolean = location_status === "ready" && has_location;
  const is_loading: boolean = location_status === "loading";

  const get_icon_style = () => {
    if (is_loading) return { Icon: Loader2, className: "animate-spin opacity-40" };
    if (can_show_location_control) return { Icon: Locate, className: "text-[#5B4636]/60 scale-90" };
    if (is_follow_mode) {
      const final_rotation_deg = (heading ?? 0) - (azimuth * 180) / Math.PI;
      return { Icon: Compass, style: { transform: `rotate(${final_rotation_deg}deg)` }, className: "text-amber-500 scale-110" };
    }
    return { Icon: Navigation, className: "text-[#5B4636]/80 group-hover:rotate-[45deg]" };
  };

  const { Icon, className, style } = get_icon_style();

  const title = can_show_location_control ? t("map.controls.move_to_location") : is_follow_mode ? t("map.controls.exit_compass_mode") : t("map.controls.start_compass_mode");

  return (
    <CircularButton
      className="w-12 h-12"
      shadow={false}
      onClick={() => void kernel_tracking(can_show_location_control ? TRACKING_RECENTER : is_follow_mode ? TRACKING_UNFOLLOW : TRACKING_FOLLOW)}
      isActive={is_follow_mode}
      disabled={!is_enabled || is_loading}
      title={title}
    >
      <Icon className={cn("w-5 h-5 transition-all duration-500 ease-out", className)} style={style} />
    </CircularButton>
  );
}
