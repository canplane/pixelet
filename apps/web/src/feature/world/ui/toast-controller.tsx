// @pxl/web/feature/world/ui/toast-controller.tsx

import { useEffect, useRef, useState } from "react";
import { useTranslation } from "react-i18next";

import { messageKey } from "@/locale/message-key.ts";
import { DevicePoseStatus, GroundStatus, Message } from "@/kernel/abi/cmd.spec.ts";
import { Gpu, useGpu } from "@/kernel/gpu.ts";
import { useOnline } from "@/kernel/online.ts";
import { useKernel } from "@/kernel/shell.ts";

import { toastDismiss, toastShow, ToastTone } from "@/shared/ui/toast/store.ts";

const enum Report {
  Location = 0,
  Orientation = 1,
  Outside = 2,
  World = 3,
  Gpu = 4,
  Network = 5,
  Server = 6,
}

/** The ready word: said once, as the first world and its companions are on screen. */
const enum Greeting {
  Waiting = 0,
  Said = 1,
  Done = 2,
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

const error = (key: string): Said => ({ key, tone: ToastTone.Error });
const loading = (key: string): Said => ({ key, tone: ToastTone.Loading });

function location(status: DevicePoseStatus, failure: Message | null): Said | null {
  if (failure !== null) return error(messageKey(failure));
  return status === DevicePoseStatus.Loading ? loading("toast.location_checking") : null;
}

/** The GPU's: none answered yet, or the one drawing is lost. */
function gpuError(gpu: Gpu): Said | null {
  return gpu === Gpu.Waiting ? error("toast.gpu_unstable") : null;
}

/** The server's: it fails the reads that reach it. */
function serverError(stall: Message | null): Said | null {
  return stall === Message.WorldReadFailing ? error(messageKey(stall)) : null;
}

/** The world coming, then its companions, then the ready word once. */
function progress(shown: boolean, companionLoading: boolean, greeting: Greeting): Said | null {
  if (!shown) return loading("toast.world_loading");
  if (companionLoading) return loading("toast.companion_loading");
  return greeting === Greeting.Said ? { key: "toast.world_ready", tone: ToastTone.Ready } : null;
}

/** Each toast is derived from what the kernel and the host say now; an error is one toast a source. */
export function ToastController() {
  const locationStatus = useKernel((s) => s.location.status);
  const locationError = useKernel((s) => s.location.error);
  useSaid(Report.Location, location(locationStatus, locationError));

  const orientationError = useKernel((s) => s.orientation.error);
  useSaid(Report.Orientation, orientationError !== null ? error(messageKey(orientationError)) : null);

  const outside = useKernel((s) => s.focus.outside);
  useSaid(Report.Outside, outside ? error("toast.outside") : null);

  const gpu = useGpu();
  const stall = useKernel((s) => s.stall);
  useSaid(Report.Gpu, gpuError(gpu));
  useSaid(Report.Network, useOnline() ? null : error("toast.network_unstable"));
  useSaid(Report.Server, serverError(stall));

  const shown = useKernel((s) => s.focus.status === GroundStatus.Success);
  const companionLoading = useKernel((s) => !s.companion.person_ready || !s.companion.pet_ready);
  const [greeting, setGreeting] = useState(Greeting.Waiting);
  const ready = shown && !companionLoading;
  useEffect(() => {
    if (ready && greeting === Greeting.Waiting) setGreeting(Greeting.Said);
  }, [ready, greeting]);
  useEffect(() => {
    if (greeting !== Greeting.Said) return;
    const timer = setTimeout(() => setGreeting(Greeting.Done), READY_MS);
    return () => clearTimeout(timer);
  }, [greeting]);
  useSaid(Report.World, progress(shown, companionLoading, greeting));

  return null;
}
