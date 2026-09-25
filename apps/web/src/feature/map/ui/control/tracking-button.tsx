// @pxl/web/feature/map/ui/control/tracking-button.tsx

import { Compass, Loader2, Locate, Navigation } from "lucide-react";
import { useTranslation } from "react-i18next";

import { DevicePoseStatus, Tracking } from "@/kernel/abi/cmd.spec.ts";
import { kernel_tracking, useKernel } from "@/kernel/shell.ts";

import { cn } from "@/shared/lib/util/cn.ts";
import { CircularButton } from "@/shared/ui/shadcn/circular-button.tsx";

export function TrackingButton() {
  const { t } = useTranslation();

  const locationStatus = useKernel((s) => s.location.status);
  const has_location = useKernel((s) => s.location.has_location);
  const heading = useKernel((s) => s.orientation.heading);
  const azimuth = useKernel((s) => s.azimuth);
  const is_follow_mode = useKernel((s) => s.is_follow_mode);
  const is_off_center = useKernel((s) => s.map.is_off_center);

  const canShowLocationControl: boolean = is_off_center && !is_follow_mode;
  const isEnabled: boolean = locationStatus === DevicePoseStatus.Ready && has_location;
  const isLoading: boolean = locationStatus === DevicePoseStatus.Loading;

  const getIconStyle = () => {
    if (isLoading) return { Icon: Loader2, className: "animate-spin opacity-40" };
    if (canShowLocationControl) return { Icon: Locate, className: "text-[#5B4636]/60 scale-90" };
    if (is_follow_mode) {
      const finalRotationDeg = (heading ?? 0) - (azimuth * 180) / Math.PI;
      return { Icon: Compass, style: { transform: `rotate(${finalRotationDeg}deg)` }, className: "text-amber-500 scale-110" };
    }
    return { Icon: Navigation, className: "text-[#5B4636]/80 group-hover:rotate-[45deg]" };
  };

  const { Icon, className, style } = getIconStyle();

  const title = canShowLocationControl ? t("map.controls.move_to_location") : is_follow_mode ? t("map.controls.exit_compass_mode") : t("map.controls.start_compass_mode");

  return (
    <CircularButton
      className="w-12 h-12"
      shadow={false}
      onClick={() => void kernel_tracking(canShowLocationControl ? Tracking.Recenter : is_follow_mode ? Tracking.Unfollow : Tracking.Follow)}
      isActive={is_follow_mode}
      disabled={!isEnabled || isLoading}
      title={title}
    >
      <Icon className={cn("w-5 h-5 transition-all duration-500 ease-out", className)} style={style} />
    </CircularButton>
  );
}
