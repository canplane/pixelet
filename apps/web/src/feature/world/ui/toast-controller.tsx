// @pxl/web/feature/world/ui/toast-controller.tsx

import { useEffect, useRef } from "react";
import { useTranslation } from "react-i18next";

import { messageKey } from "@/locale/message-key.ts";
import { DevicePoseStatus, GroundStatus } from "@/kernel/abi/cmd.spec.ts";
import { useGpuAbsent } from "@/kernel/gpu.ts";
import { useKernel } from "@/kernel/shell.ts";

import { toastDismiss, toastFor, toastShow, ToastTone } from "@/shared/ui/toast/store.ts";

const enum Report {
  Location = 0,
  Orientation = 1,
  Outside = 2,
  World = 3,
  Ready = 4,
}

const READY_MS = 3000;

/** A toast's words, as a locale key. */
interface Said {
  key: string;
  tone: ToastTone;
}

/** The words, as `t` reads them now: react-i18next's new `t` on a language change re-showed toasts. */
function useWords(): (key: string) => string {
  const { t } = useTranslation();
  const tRef = useRef(t);
  tRef.current = t;
  return useRef((key: string) => tRef.current(key)).current;
}

/** A state's toast: shown while `said` is, gone with it. */
function useSaid(id: Report, said: Said | null): void {
  const words = useWords();
  const key = said?.key ?? null;
  const tone = said?.tone ?? null;
  useEffect(() => {
    if (key === null || tone === null) toastDismiss(id);
    else toastShow(id, { message: words(key), tone });
  }, [id, key, tone, words]);
}

/** Each toast but the ready word is a state's, derived from what the kernel and the host say now. */
export function ToastController() {
  const words = useWords();

  const location = useKernel((s) => s.location.status);
  const locationError = useKernel((s) => s.location.error);
  useSaid(
    Report.Location,
    locationError !== null ? { key: messageKey(locationError), tone: ToastTone.Error } : location === DevicePoseStatus.Loading ? { key: "toast.location_checking", tone: ToastTone.Loading } : null,
  );

  const orientationError = useKernel((s) => s.orientation.error);
  useSaid(Report.Orientation, orientationError !== null ? { key: messageKey(orientationError), tone: ToastTone.Error } : null);

  const outside = useKernel((s) => s.focus.outside);
  useSaid(Report.Outside, outside ? { key: "toast.outside", tone: ToastTone.Error } : null);

  // Why the world is not on screen outranks that it is coming; a stall is retried, so it waits.
  // Outside the service area nothing is coming, which its own toast says.
  const gpuAbsent = useGpuAbsent();
  const stall = useKernel((s) => s.stall);
  const shown = useKernel((s) => s.focus.status === GroundStatus.Success);
  const companionLoading = useKernel((s) => !s.companion.person_ready || !s.companion.pet_ready);
  useSaid(
    Report.World,
    gpuAbsent
      ? { key: "unsupported.gpu", tone: ToastTone.Error }
      : stall !== null
      ? { key: messageKey(stall), tone: ToastTone.Loading }
      : !shown && !outside
      ? { key: "toast.world_loading", tone: ToastTone.Loading }
      : companionLoading
      ? { key: "toast.companion_loading", tone: ToastTone.Loading }
      : null,
  );

  // The one event: the first world on screen, its companions with it.
  const ready = shown && !companionLoading;
  const announced = useRef(false);
  useEffect(() => {
    if (!ready || announced.current) return;
    announced.current = true;
    toastFor(Report.Ready, { message: words("toast.world_ready"), tone: ToastTone.Ready }, READY_MS);
  }, [ready, words]);

  return null;
}
