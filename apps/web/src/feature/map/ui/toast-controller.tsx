// @pxl/web/feature/map/ui/toast-controller.tsx

import { useEffect, useRef } from "react";
import { AnimatePresence, motion } from "framer-motion";
import { CheckCircle2, Loader2, MapPinOff } from "lucide-react";
import { useTranslation } from "react-i18next";

import { messageKey as locationMessageKey } from "@/locale/message-key.ts";
import { DevicePoseStatus, MapStatus } from "@/kernel/abi/cmd.spec.ts";
import { useKernel } from "@/kernel/shell.ts";

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
  const locationStatus = useKernel((s) => s.location.status);
  // Rendered through t() here (not resolved to a message where the error
  // fires) because i18next loads its catalogue over the network — an
  // imperative t() at sensor-error/startup time would freeze the raw key
  // in the store and never follow a later language change.
  const locationError = useKernel((s) => s.location.error);
  const locationErrorKey: string | null = locationError === null ? null : locationMessageKey(locationError);

  const mapStatus = useKernel((s) => s.map.status);
  // The L3 page the map centre falls in — what "the area I am looking at"
  // means to the kernel, and the key the world toast is announced per.
  const mapPage = useKernel((s) => s.map.page);
  // Merged with mapStatus below into a single "world-status" toast, not a
  // toast of its own — both represent the same thing to a viewer ("is the
  // world I'm looking at actually ready yet"), unlike locationStatus
  // (device GPS sensor state, a genuinely separate concern that keeps its
  // own toast).
  const companion_loading: boolean = useKernel((s) => !s.companion.person_ready || !s.companion.pet_ready);

  const shownLocationToastRef = useRef(false);
  // t via a ref, not a dependency — react-i18next hands back a new function
  // identity on every language change, so listing it as a dependency re-ran
  // these effects and re-showed a toast just from switching language.
  const tRef = useRef(t);
  tRef.current = t;

  useEffect(() => {
    const isLoading = locationStatus === DevicePoseStatus.Loading;
    const isError = locationStatus === DevicePoseStatus.Error;

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
        message={isError ? (locationErrorKey ? tRef.current(locationErrorKey) : tRef.current("toast.location_weak_signal")) : tRef.current("toast.location_checking")}
        statusColor={isError ? "bg-red-500" : "bg-[#34C759]/30"}
        icon={<StatusIcon>{isError ? <MapPinOff key="location-error-icon" className="w-4 h-4 text-red-500" /> : LoadingIcon}</StatusIcon>}
        backgroundColor={isError ? "bg-red-50" : undefined}
        className={isError ? "border-red-200 shadow-[0_6px_20px_rgba(220,38,38,0.15)]" : undefined}
      />
    ));
  }, [locationStatus, locationErrorKey]);

  // Tracks only "was the combined loading/error state active last run" —
  // the transition into !isLoading && !isError is what distinguishes
  // "just finished, flash success" from "never started, nothing to show."
  const wasWorldLoadingRef = useRef(false);
  // The last page announced as ready. `undefined` rather than null so that the
  // very first announcement is not swallowed by a centre that has none yet.
  const announcedPageRef = useRef<bigint | null | undefined>(undefined);
  const shownWorldToastRef = useRef(false);
  // The ready toast's own timer, held here rather than returned as the
  // effect's cleanup: the effect re-runs whenever the page changes, and a
  // cleanup that cancelled the timer would leave the toast standing forever.
  const dismissTimerRef = useRef<ReturnType<typeof setTimeout> | null>(null);
  const dropWorldToast = () => {
    if (dismissTimerRef.current !== null) {
      clearTimeout(dismissTimerRef.current);
      dismissTimerRef.current = null;
    }
    if (!shownWorldToastRef.current) return;
    toastDismiss("world-status");
    shownWorldToastRef.current = false;
  };
  useEffect(() => dropWorldToast, []);
  // Once the companion has completed its very first load, every later
  // person_ready/pet_ready dip is from the user swapping a variant in
  // settings (Walker remounts on a key change, so a fresh instance briefly
  // reports not-ready) — a costume change, not the world going unready —
  // so companion_loading stops feeding this toast after that point.
  const companionFirstLoadDoneRef = useRef(false);
  useEffect(() => {
    const isMapLoading = mapStatus === MapStatus.Loading;
    const isError = mapStatus === MapStatus.Error;
    const relevantCompanionLoading = companion_loading && !companionFirstLoadDoneRef.current;
    if (!companion_loading) companionFirstLoadDoneRef.current = true;
    const isLoading = isMapLoading || relevantCompanionLoading;

    // A page already announced is one the user has been told about, and what
    // reaches the terrain queue for it afterwards is the fetcher asking again
    // for a tile that did not arrive — every OCEAN_RETRY_MS, for as long as it
    // keeps not arriving (world/pyramid.rs). The queue filling and
    // draining on that beat is not the world loading and becoming ready, and
    // announcing it as such replayed this toast every two seconds forever.
    // An error still speaks, since that is news whatever page it is on.
    const alreadyAnnounced = mapPage === announcedPageRef.current;

    if (isLoading || isError) {
      wasWorldLoadingRef.current = true;
      if (alreadyAnnounced && !isError) {
        dropWorldToast();
        return;
      }
      // Map load takes priority in the message when both are in flight —
      // it's the more foundational "is there a world to stand on at all"
      // concern; companion_loading only surfaces once mapStatus has
      // nothing left to report.
      const messageKey = isError ? "toast.map_load_error" : isMapLoading ? "toast.map_loading" : "toast.companion_loading";
      shownWorldToastRef.current = true;
      toastCustom("world-status", ({ index, stackSize, isRemoving }) => (
        <Toast
          id="world-status"
          index={index}
          stackSize={stackSize}
          isRemoving={isRemoving}
          message={tRef.current(messageKey)}
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
    if (alreadyAnnounced) {
      dropWorldToast();
      return;
    }
    announcedPageRef.current = mapPage;
    shownWorldToastRef.current = true;
    toastCustom("world-status", ({ index, stackSize, isRemoving }) => (
      <Toast
        id="world-status"
        index={index}
        stackSize={stackSize}
        isRemoving={isRemoving}
        message={tRef.current("toast.world_ready")}
        statusColor="bg-[#34C759]"
        icon={<StatusIcon>{SuccessIcon}</StatusIcon>}
      />
    ));
    dismissTimerRef.current = setTimeout(() => {
      dismissTimerRef.current = null;
      toastDismiss("world-status");
      shownWorldToastRef.current = false;
    }, 3000);
  }, [mapStatus, companion_loading, mapPage]);

  return null;
}
