// @pxl/native/bracket.ts

/** The device's bracket: what the client asks of the app (@pxl/client/bracket.tsx), answered by the app's native module (module.ts) and the kernel linked into it (pxl-native). */

import type { Bracket } from "@pxl/client/bracket.tsx";
import { HANDHELD } from "./device.ts";
import { gpuBoot } from "./gpu.ts";
import { openGuest } from "./guest.ts";
import { preferredLanguages, showLanguage } from "./locale.ts";
import { onlineWatch } from "./online.ts";
import { ORIGIN, originGet } from "./origin.ts";
import { useSafeArea } from "./safe-area.ts";
import { copyText, saveFile } from "./save.ts";
import {
  geolocationStart,
  geolocationStop,
  geolocationSupported,
  orientationListen,
  orientationNeedsPermission,
  orientationPermit,
  orientationRemove,
  orientationSilenceIsLoss,
  orientationSupported,
  permissionFollow,
  sensorsHear,
} from "./sensor.ts";
import { storageGet, storageSet } from "./storage.ts";
import { textRaster, textWidth } from "./text.ts";
import { useViewportTop, useVisualViewport, useWindowSize } from "./viewport.ts";
import { CaseSurface, WorldSurface } from "./surface.tsx";

export const DEVICE: Bracket = {
  openGuest,
  gpuBoot,
  HANDHELD,
  storageGet,
  storageSet,
  textWidth,
  textRaster,
  ORIGIN,
  originGet,
  onlineWatch,
  geolocationSupported,
  orientationSupported,
  orientationNeedsPermission,
  orientationSilenceIsLoss,
  orientationPermit,
  permissionFollow,
  geolocationStart,
  geolocationStop,
  orientationListen,
  orientationRemove,
  sensorsHear,
  saveFile,
  copyText,
  preferredLanguages,
  showLanguage,
  useSafeArea,
  useWindowSize,
  useVisualViewport,
  useViewportTop,
  WorldSurface,
  CaseSurface,
};
