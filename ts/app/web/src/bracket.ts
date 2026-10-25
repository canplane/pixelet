// @pxl/web/bracket.ts

/** The page's bracket: what the client asks of a browser tab (@pxl/client/bracket.tsx), answered by the browser's own APIs. */

import type { Bracket } from "@pxl/client/bracket.tsx";
import * as device from "./device.ts";
import * as gpu from "./gpu.ts";
import * as guest from "./guest.ts";
import * as locale from "./locale.ts";
import * as online from "./online.ts";
import * as origin from "./origin.ts";
import * as sensor from "./sensor.ts";
import * as share from "./share.ts";
import * as storage from "./storage.ts";
import * as surface from "./surface.tsx";
import * as text from "./text.ts";
import { useSafeArea } from "./safe-area.ts";
import { useViewportTop, useVisualViewport, useWindowSize } from "./viewport.ts";

export const page: Bracket = {
  device,
  guest,
  gpu,
  storage,
  text,
  origin,
  online,
  sensor,
  share,
  locale,
  view: { useSafeArea, useWindowSize, useVisualViewport, useViewportTop },
  surface,
};
