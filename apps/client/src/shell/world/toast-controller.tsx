// @pxl/client/shell/world/toast-controller.tsx

import { useEffect, useRef, useState } from "react";
import { useTranslation } from "react-i18next";

import { messageKey } from "@/shell/locale/message-key.ts";
import { DevicePoseStatus, GroundStatus, Message } from "@/host/abi/cmd.spec.ts";
import { Gpu, useGpu } from "@/host/gpu.ts";
import { useOnline } from "@/host/online.ts";
import { useKernel } from "@/host/shell.ts";

import { toastDismiss, toastShow, ToastTone } from "@/shell/ui/toast/store.ts";

/** Each report's toast; of one tone, by rank: a cause before what it holds
 * up, the device's before the server's, the world's before a sensor's. */
const enum Report {
  Gpu = 0,
  Network = 1,
  Server = 2,
  World = 3,
  Location = 4,
  Orientation = 5,
}

/** The ready word: said once, as the first world and its companions are on screen. */
const enum Greeting {
  Waiting = 0,
  Said = 1,
  Done = 2,
}

const READY_MS = 3000;

/** How long the server's toast stays once its cause clears: a server failing
 * one read and answering the next would show and hide it each turn, the
 * stack trading its front card. A product number. */
const SERVER_HOLD_MS = 3000;

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

/** A state's toast: shown while `said` is, gone with it, `holdMs` later. */
function useSaid(id: Report, said: Said | null, holdMs = 0): void {
  const words = useWords();
  const key = said?.key ?? null;
  const tone = said?.tone ?? null;
  useEffect(() => {
    if (key !== null && tone !== null) {
      toastShow(id, { message: words(key), tone });
      return;
    }
    if (holdMs === 0) {
      toastDismiss(id);
      return;
    }
    const timer = setTimeout(() => toastDismiss(id), holdMs);
    return () => clearTimeout(timer);
  }, [id, key, tone, words, holdMs]);
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

/** The world's one toast: none there, or it coming, then its companions, then the ready word once. */
function world(outside: boolean, shown: boolean, companionLoading: boolean, greeting: Greeting): Said | null {
  if (outside) return error("toast.outside");
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

  const gpu = useGpu();
  const stall = useKernel((s) => s.stall);
  useSaid(Report.Gpu, gpuError(gpu));
  useSaid(Report.Network, useOnline() ? null : error("toast.network_unstable"));
  useSaid(Report.Server, serverError(stall), SERVER_HOLD_MS);

  const outside = useKernel((s) => s.focus.outside);
  const shown = useKernel((s) => s.focus.status === GroundStatus.Success);
  const companionLoading = useKernel((s) => !s.companion.person_ready || !s.companion.pet_ready);
  const [greeting, setGreeting] = useState(Greeting.Waiting);
  const ready = !outside && shown && !companionLoading;
  useEffect(() => {
    if (ready && greeting === Greeting.Waiting) setGreeting(Greeting.Said);
    // Cut short, it is not said again.
    if (!ready && greeting === Greeting.Said) setGreeting(Greeting.Done);
  }, [ready, greeting]);
  useEffect(() => {
    if (greeting !== Greeting.Said) return;
    const timer = setTimeout(() => setGreeting(Greeting.Done), READY_MS);
    return () => clearTimeout(timer);
  }, [greeting]);
  useSaid(Report.World, world(outside, shown, companionLoading, greeting));

  return null;
}
