// @pxl/native/bracket.ts

/** The device's bracket: what the client asks of the app (@pxl/client/bracket.tsx), answered by the app's native module (module.ts) and the kernel linked into it (pxl-native). */

import type { Bracket } from "@pxl/client/bracket.tsx";
import * as device from "./device.ts";
import * as gpu from "./gpu.ts";
import * as guest from "./guest.ts";
import * as locale from "./locale.ts";
import * as longFrame from "./long-frame.ts";
import * as online from "./online.ts";
import * as origin from "./origin.ts";
import * as sensor from "./sensor.ts";
import * as share from "./share.ts";
import * as storage from "./storage.ts";
import * as surface from "./surface.tsx";
import * as text from "./text.ts";
import * as visibility from "./visibility.ts";
import { useSafeArea } from "./safe-area.ts";
import {
  useViewportTop,
  useVisualViewport,
  useWindowSize,
} from "./viewport.ts";

export const app: Bracket = {
  device,
  guest,
  gpu,
  storage,
  text,
  origin,
  online,
  visibility,
  sensor,
  share,
  locale,
  longFrame,
  view: { useSafeArea, useWindowSize, useVisualViewport, useViewportTop },
  surface,
};
