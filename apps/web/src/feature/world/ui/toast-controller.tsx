// @pxl/web/feature/world/ui/toast-controller.tsx

import { useEffect, useRef } from "react";
import { useTranslation } from "react-i18next";

import { GroundStatus } from "@/kernel/abi/cmd.spec.ts";
import { useKernel } from "@/kernel/shell.ts";

import { toastFor, toastShow, ToastTone } from "@/shared/ui/toast/store.ts";

/** What a toast tells of: an event, said and gone. A state that holds is the status band's, the load cover's or its control's. */
export const enum Report {
  Location = 0,
  World = 1,
}

const READY_MS = 3000;

export function ToastController() {
  const { t } = useTranslation();
  // A ref: react-i18next's new `t` on a language change re-showed toasts.
  const tRef = useRef(t);
  tRef.current = t;

  // The first world on screen is announced once, its companions with it.
  const shown = useKernel((s) => s.focus.status === GroundStatus.Success);
  const companionLoading = useKernel((s) => !s.companion.person_ready || !s.companion.pet_ready);
  const announced = useRef(false);

  useEffect(() => {
    if (announced.current || !shown) return;
    if (companionLoading) {
      toastShow(Report.World, { message: tRef.current("toast.companion_loading"), tone: ToastTone.Loading });
      return;
    }
    announced.current = true;
    toastFor(Report.World, { message: tRef.current("toast.world_ready"), tone: ToastTone.Ready }, READY_MS);
  }, [shown, companionLoading]);

  return null;
}
