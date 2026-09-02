// @pxl/web/modules/map/ui/toast/toast_controller.tsx

import { useEffect, useRef } from "react";
import { AnimatePresence, motion } from "framer-motion";
import { CheckCircle2, Loader2, MapPinOff } from "lucide-react";
import { useTranslation } from "react-i18next";

import { DevicePoseStatus, useDeviceLocationStore } from "@/modules/device/pose/store.ts";
import { MapStatus, useMapStore } from "@/modules/map/store.ts";
import { useCompanionLoadStore } from "@/modules/world/characters/store.ts";

import { Toast } from "@/shared/ui/toast/toast.tsx";
import { toastCustom, toastDismiss } from "@/shared/ui/toast/store.ts";

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

export function ToastController() {
  const { t } = useTranslation();
  const location_status: DevicePoseStatus = useDeviceLocationStore((state) => state.status);
  // Rendered through t() here (not resolved to a message where the error
  // fires) because i18next loads its catalogue over the network — an
  // imperative t() at sensor-error/startup time would freeze the raw key
  // in the store and never follow a later language change.
  const location_error_key: string | null = useDeviceLocationStore((state) => state.error_key);

  const map_status: MapStatus = useMapStore((state) => state.map_status);
  // Merged with map_status below into a single "world-status" toast, not a
  // toast of its own — both represent the same thing to a viewer ("is the
  // world I'm looking at actually ready yet"), unlike location_status
  // (device GPS sensor state, a genuinely separate concern that keeps its
  // own toast).
  const companion_loading: boolean = useCompanionLoadStore((state) => !state.person_ready || !state.pet_ready);

  const shownLocationToastRef = useRef(false);
  // t via a ref, not a dependency — react-i18next hands back a new function
  // identity on every language change, so listing it as a dependency re-ran
  // these effects and re-showed a toast just from switching language.
  const t_ref = useRef(t);
  t_ref.current = t;

  useEffect(() => {
    const isLoading = location_status === DevicePoseStatus.LOADING;
    const isError = location_status === DevicePoseStatus.ERROR;

    if (!isLoading && !isError) {
      if (shownLocationToastRef.current) {
        toastDismiss("location-status");
        shownLocationToastRef.current = false;
      }
      return;
    }

    shownLocationToastRef.current = true;
    toastCustom("location-status", ({ index, stackSize, isRemoving }) => (
      <Toast
        id="location-status"
        index={index}
        stackSize={stackSize}
        isRemoving={isRemoving}
        message={isError ? (location_error_key ? t_ref.current(location_error_key) : t_ref.current("toast.location_weak_signal")) : t_ref.current("toast.location_checking")}
        statusColor={isError ? "bg-red-500" : "bg-[#34C759]/30"}
        icon={<StatusIcon>{isError ? <MapPinOff key="location-error-icon" className="w-4 h-4 text-red-500" /> : LoadingIcon}</StatusIcon>}
        backgroundColor={isError ? "bg-red-50" : undefined}
        className={isError ? "border-red-200 shadow-[0_6px_20px_rgba(220,38,38,0.15)]" : undefined}
      />
    ));
  }, [location_status, location_error_key]);

  // Tracks only "was the combined loading/error state active last run" —
  // the transition into !isLoading && !isError is what distinguishes
  // "just finished, flash success" from "never started, nothing to show."
  const wasWorldLoadingRef = useRef(false);
  // Once the companion has completed its very first load, every later
  // person_ready/pet_ready dip is from the user swapping a variant in
  // settings (Walker remounts on a key change, so a fresh instance briefly
  // reports not-ready) — a costume change, not the world going unready —
  // so companion_loading stops feeding this toast after that point.
  const companionFirstLoadDoneRef = useRef(false);
  useEffect(() => {
    const isMapLoading = map_status === MapStatus.LOADING;
    const isError = map_status === MapStatus.ERROR;
    const relevantCompanionLoading = companion_loading && !companionFirstLoadDoneRef.current;
    if (!companion_loading) companionFirstLoadDoneRef.current = true;
    const isLoading = isMapLoading || relevantCompanionLoading;

    if (isLoading || isError) {
      wasWorldLoadingRef.current = true;
      // Map load takes priority in the message when both are in flight —
      // it's the more foundational "is there a world to stand on at all"
      // concern; companion_loading only surfaces once map_status has
      // nothing left to report.
      const message_key = isError ? "toast.map_load_error" : isMapLoading ? "toast.map_loading" : "toast.companion_loading";
      toastCustom("world-status", ({ index, stackSize, isRemoving }) => (
        <Toast
          id="world-status"
          index={index}
          stackSize={stackSize}
          isRemoving={isRemoving}
          message={t_ref.current(message_key)}
          statusColor={isError ? "bg-red-500" : "bg-[#34C759]/30"}
          icon={<StatusIcon>{isError ? <MapPinOff key="world-error-icon" className="w-4 h-4 text-red-500" /> : LoadingIcon}</StatusIcon>}
          backgroundColor={isError ? "bg-red-50" : undefined}
          className={isError ? "border-red-200 shadow-[0_6px_20px_rgba(220,38,38,0.15)]" : undefined}
        />
      ));
      return;
    }

    if (!wasWorldLoadingRef.current) return;
    wasWorldLoadingRef.current = false;
    toastCustom("world-status", ({ index, stackSize, isRemoving }) => (
      <Toast
        id="world-status"
        index={index}
        stackSize={stackSize}
        isRemoving={isRemoving}
        message={t_ref.current("toast.world_ready")}
        statusColor="bg-[#34C759]"
        icon={<StatusIcon>{SuccessIcon}</StatusIcon>}
      />
    ));
    const timer = setTimeout(() => toastDismiss("world-status"), 3000);
    return () => clearTimeout(timer);
  }, [map_status, companion_loading]);

  return null;
}
