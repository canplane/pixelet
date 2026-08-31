// @pxl/web/modules/map/ui/toast/toast_controller.tsx

import { useEffect, useRef } from "react";
import { AnimatePresence, motion } from "framer-motion";
import { CheckCircle2, Loader2, MapPinOff } from "lucide-react";
import { useTranslation } from "react-i18next";

import { DevicePoseStatus, useDeviceLocationStore } from "@/modules/device/pose/store.ts";
import { MapStatus, useMapStore } from "@/modules/map/store.ts";

import { Toast } from "@/shared/ui/toast/toast.tsx";
import { toast_custom, toast_dismiss } from "@/shared/ui/toast/store.ts";

const LoadingIcon = (
  <motion.div
    key="loading-icon"
    initial={{ opacity: 0, scale: 0.8 }}
    animate={{ opacity: 1, scale: 1 }}
    exit={{ opacity: 0, scale: 0.5 }}
    transition={{ duration: 0.2 }}
  >
    <Loader2 className="w-4 h-4 text-[#5B4636] animate-spin-slow stroke-[2.5px]" />
  </motion.div>
);

const SuccessIcon = (
  <motion.div
    key="success-icon"
    initial={{ opacity: 0, scale: 0.3, rotate: -20 }}
    animate={{ opacity: 1, scale: 1, rotate: 0 }}
    transition={{ type: "spring", damping: 12, stiffness: 260, mass: 0.8 }}
  >
    <CheckCircle2 className="w-4 h-4 text-[#34C759] stroke-[2.5px]" />
  </motion.div>
);

function StatusIcon({ children }: { children: React.ReactNode }) {
  return (
    <div className="relative flex items-center justify-center w-4 h-4">
      <AnimatePresence mode="wait" initial={false}>
        {children}
      </AnimatePresence>
    </div>
  );
}

export function MapToastController() {
  const { t } = useTranslation();
  const location_status: DevicePoseStatus = useDeviceLocationStore((state) => state.status);
  // Rendered through t() here (not resolved to a message where the error
  // fires) because i18next loads its catalogue over the network — an
  // imperative t() at sensor-error/startup time would freeze the raw key
  // in the store and never follow a later language change.
  const location_error_key: string | null = useDeviceLocationStore((state) => state.error_key);

  const map_status: MapStatus = useMapStore((state) => state.map_status);

  const shown_location_toast_ref = useRef(false);
  // t via a ref, not a dependency — react-i18next hands back a new function
  // identity on every language change, so listing it as a dependency re-ran
  // these effects and re-showed a toast just from switching language.
  const t_ref = useRef(t);
  t_ref.current = t;

  useEffect(() => {
    const is_loading = location_status === DevicePoseStatus.LOADING;
    const is_error = location_status === DevicePoseStatus.ERROR;

    if (!is_loading && !is_error) {
      if (shown_location_toast_ref.current) {
        toast_dismiss("location-status");
        shown_location_toast_ref.current = false;
      }
      return;
    }

    shown_location_toast_ref.current = true;
    toast_custom("location-status", ({ index, stack_size, is_removing }) => (
      <Toast
        id="location-status"
        index={index}
        stack_size={stack_size}
        is_removing={is_removing}
        message={is_error ? (location_error_key ? t_ref.current(location_error_key) : t_ref.current("toast.location_weak_signal")) : t_ref.current("toast.location_checking")}
        statusColor={is_error ? "bg-red-500" : "bg-[#34C759]/30"}
        icon={<StatusIcon>{is_error ? <MapPinOff key="location-error-icon" className="w-4 h-4 text-red-500" /> : LoadingIcon}</StatusIcon>}
        backgroundColor={is_error ? "bg-red-50" : undefined}
        className={is_error ? "border-red-200 shadow-[0_6px_20px_rgba(220,38,38,0.15)]" : undefined}
      />
    ));
  }, [location_status, location_error_key]);

  const shown_map_toast_ref = useRef(false);
  useEffect(() => {
    const is_loading = map_status === MapStatus.LOADING;
    const is_error = map_status === MapStatus.ERROR;
    const is_success = map_status === MapStatus.SUCCESS;

    if (!is_loading && !is_error && !is_success) {
      if (shown_map_toast_ref.current) {
        toast_dismiss("map-status");
        shown_map_toast_ref.current = false;
      }
      return;
    }

    shown_map_toast_ref.current = true;
    toast_custom("map-status", ({ index, stack_size, is_removing }) => (
      <Toast
        id="map-status"
        index={index}
        stack_size={stack_size}
        is_removing={is_removing}
        message={is_error ? t_ref.current("toast.map_load_error") : is_success ? t_ref.current("toast.map_load_success") : t_ref.current("toast.map_loading")}
        statusColor={is_error ? "bg-red-500" : is_success ? "bg-[#34C759]" : "bg-[#34C759]/30"}
        icon={<StatusIcon>{is_error ? <MapPinOff key="map-error-icon" className="w-4 h-4 text-red-500" /> : is_success ? SuccessIcon : LoadingIcon}</StatusIcon>}
        backgroundColor={is_error ? "bg-red-50" : undefined}
        className={is_error ? "border-red-200 shadow-[0_6px_20px_rgba(220,38,38,0.15)]" : undefined}
      />
    ));

    if (!is_success) return;
    const timer = setTimeout(() => toast_dismiss("map-status"), 3000);
    return () => clearTimeout(timer);
  }, [map_status]);

  return null;
}
