// @pxl/web/module/map/ui/overlay/sensor_status.tsx

import { Compass, Locate } from "lucide-react";
import { useTranslation } from "react-i18next";

import { kernel_enable_sensors, type PoseStatus, useKernel } from "@/wasm/kernel.ts";

import { cn } from "@/shared/lib/util/cn.ts";

function status_class(status: PoseStatus): string {
  switch (status) {
    case "ready":
      return "text-emerald-500";
    case "loading":
      return "text-amber-400 animate-pulse";
    case "error":
      return "text-red-400";
    case "idle":
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

  const location_status = useKernel((s) => s.location.status);
  const orientation_status = useKernel((s) => s.orientation.status);

  const can_request: boolean = location_status !== "ready" || orientation_status !== "ready";

  const icons = (
    <>
      <Locate className={cn("w-3.5 h-3.5 transition-colors duration-500", status_class(location_status))} strokeWidth={2.5} />
      <Compass className={cn("w-3.5 h-3.5 transition-colors duration-500", status_class(orientation_status))} strokeWidth={2.5} />
    </>
  );

  if (!can_request) return <div className="flex items-center gap-2">{icons}</div>;

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
