// @pxl/web/feature/map/ui/overlay/sensor-status.tsx

import { Compass, Locate } from "lucide-react";
import { useTranslation } from "react-i18next";

import { DevicePoseStatus } from "@/kernel/abi/cmd.spec.ts";
import { kernel_enable_sensors, useKernel } from "@/kernel/shell.ts";

import { cn } from "@/shared/lib/util/cn.ts";

function statusClass(status: DevicePoseStatus): string {
  switch (status) {
    case DevicePoseStatus.Ready:
      return "text-emerald-500";
    case DevicePoseStatus.Loading:
      return "text-amber-400 animate-pulse";
    case DevicePoseStatus.Error:
      return "text-red-400";
    case DevicePoseStatus.Idle:
    default:
      return "text-[#5B4636]/30";
  }
}

// DeviceOrientationEvent.requestPermission() only runs inside a direct user
// gesture on iOS Safari, and the kernel deliberately doesn't auto-start it
// there, so this control and the tracking button are the routes to that
// permission.
export function SensorStatus() {
  const { t } = useTranslation();

  const locationStatus = useKernel((s) => s.location.status);
  const orientationStatus = useKernel((s) => s.orientation.status);

  const canRequest: boolean = locationStatus !== DevicePoseStatus.Ready || orientationStatus !== DevicePoseStatus.Ready;

  const icons = (
    <>
      <Locate className={cn("w-3.5 h-3.5 transition-colors duration-500", statusClass(locationStatus))} strokeWidth={2.5} />
      <Compass className={cn("w-3.5 h-3.5 transition-colors duration-500", statusClass(orientationStatus))} strokeWidth={2.5} />
    </>
  );

  if (!canRequest) return <div className="flex items-center gap-2">{icons}</div>;

  return (
    <button
      type="button"
      onClick={kernel_enable_sensors}
      className="flex items-center gap-2 -m-2 p-2 cursor-pointer pointer-events-auto"
      title={t("map.sensors.enable")}
      aria-label={t("map.sensors.enable")}
    >
      {icons}
    </button>
  );
}
