// @pxl/web/feature/map/ui/toast-controller.tsx

import { useEffect, useRef } from "react";
import { useTranslation } from "react-i18next";

import { messageKey as locationMessageKey } from "@/locale/message-key.ts";
import { DevicePoseStatus, MapStatus } from "@/kernel/abi/cmd.spec.ts";
import { useKernel } from "@/kernel/shell.ts";

import { toastDismiss, toastShow, ToastTone } from "@/shared/ui/toast/store.ts";

const enum Report {
  Location = 0,
  World = 1,
}

export function ToastController() {
  const { t } = useTranslation();
  const locationStatus = useKernel((s) => s.location.status);
  // Translated here, not where the error fires, so a language change follows.
  const locationError = useKernel((s) => s.location.error);
  const locationErrorKey: string | null = locationError === null ? null : locationMessageKey(locationError);

  const mapStatus = useKernel((s) => s.map.status);
  const mapPage = useKernel((s) => s.map.page);
  // Folded into the world toast, not its own.
  const companionLoading: boolean = useKernel((s) => !s.companion.person_ready || !s.companion.pet_ready);

  const shownLocationToastRef = useRef(false);
  // A ref: react-i18next's new `t` on a language change re-showed toasts.
  const tRef = useRef(t);
  tRef.current = t;

  useEffect(() => {
    const isLoading = locationStatus === DevicePoseStatus.Loading;
    const isError = locationStatus === DevicePoseStatus.Error;

    if (!isLoading && !isError) {
      if (shownLocationToastRef.current) {
        toastDismiss(Report.Location);
        shownLocationToastRef.current = false;
      }
      return;
    }

    shownLocationToastRef.current = true;
    toastShow(Report.Location, {
      message: isError ? (locationErrorKey ? tRef.current(locationErrorKey) : tRef.current("toast.location_weak_signal")) : tRef.current("toast.location_checking"),
      tone: isError ? ToastTone.Error : ToastTone.Loading,
    });
  }, [locationStatus, locationErrorKey]);

  const wasWorldLoadingRef = useRef(false);
  // `undefined`, not null, so the first announcement is not swallowed.
  const announcedPageRef = useRef<bigint | null | undefined>(undefined);
  const shownWorldToastRef = useRef(false);
  // Not the effect's cleanup: the effect re-runs per page, which would leave the toast up forever.
  const dismissTimerRef = useRef<ReturnType<typeof setTimeout> | null>(null);
  const dropWorldToast = () => {
    if (dismissTimerRef.current !== null) {
      clearTimeout(dismissTimerRef.current);
      dismissTimerRef.current = null;
    }
    if (!shownWorldToastRef.current) return;
    toastDismiss(Report.World);
    shownWorldToastRef.current = false;
  };
  useEffect(() => dropWorldToast, []);
  // After the first load, companion dips are variant swaps, not the world loading.
  const companionFirstLoadDoneRef = useRef(false);
  useEffect(() => {
    const isMapLoading = mapStatus === MapStatus.Loading;
    const isError = mapStatus === MapStatus.Error;
    const relevantCompanionLoading = companionLoading && !companionFirstLoadDoneRef.current;
    if (!companionLoading) companionFirstLoadDoneRef.current = true;
    const isLoading = isMapLoading || relevantCompanionLoading;

    // Retries for an announced page are not the world loading; announcing them
    // replayed this toast every two seconds forever. Errors still speak.
    const alreadyAnnounced = mapPage === announcedPageRef.current;

    if (isLoading || isError) {
      wasWorldLoadingRef.current = true;
      if (alreadyAnnounced && !isError) {
        dropWorldToast();
        return;
      }
      const messageKey = isError ? "toast.map_load_error" : isMapLoading ? "toast.map_loading" : "toast.companion_loading";
      shownWorldToastRef.current = true;
      toastShow(Report.World, { message: tRef.current(messageKey), tone: isError ? ToastTone.Error : ToastTone.Loading });
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
    toastShow(Report.World, { message: tRef.current("toast.world_ready"), tone: ToastTone.Ready });
    dismissTimerRef.current = setTimeout(() => {
      dismissTimerRef.current = null;
      toastDismiss(Report.World);
      shownWorldToastRef.current = false;
    }, 3000);
  }, [mapStatus, companionLoading, mapPage]);

  return null;
}
