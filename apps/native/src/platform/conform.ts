// @pxl/native/platform/conform.ts

/**
 * The device's platform held to the web's: every module here exports what
 * the web's does, of the type the web's has. The shell is type-checked with
 * `@platform/` as the web's (apps/web/deno.jsonc), so a device's module that
 * lacked a name, or took another shape, would pass the check and fail only
 * when Metro ran it; root deno.jsonc's check:native checks this file and fails
 * there instead. A module may export more than the web's does, never less.
 *
 * Nothing imports this. It is a check, not code, and Metro never sees it.
 */

import type * as WebAboveKeyboard from "../../../web/src/platform/web/above-keyboard.tsx";
import type * as WebCore from "../../../web/src/platform/web/core.ts";
import type * as WebDevOverlay from "../../../web/src/platform/web/dev-overlay.tsx";
import type * as WebGlass from "../../../web/src/platform/web/glass.tsx";
import type * as WebGpu from "../../../web/src/platform/web/gpu.ts";
import type * as WebHost from "../../../web/src/platform/web/host.ts";
import type * as WebImage from "../../../web/src/platform/web/image.ts";
import type * as WebLocale from "../../../web/src/platform/web/locale.ts";
import type * as WebOrigin from "../../../web/src/platform/web/origin.ts";
import type * as WebSafeArea from "../../../web/src/platform/web/safe-area.ts";
import type * as WebSensor from "../../../web/src/platform/web/sensor.ts";
import type * as WebStorage from "../../../web/src/platform/web/storage.ts";
import type * as WebSurface from "../../../web/src/platform/web/surface.tsx";
import type * as WebText from "../../../web/src/platform/web/text.ts";
import type * as WebVisualViewport from "../../../web/src/platform/web/visual-viewport.ts";

import * as AboveKeyboard from "./above-keyboard.tsx";
import * as Core from "./core.ts";
import * as DevOverlay from "./dev-overlay.tsx";
import * as Glass from "./glass.tsx";
import * as Gpu from "./gpu.ts";
import * as Host from "./host.ts";
import * as Image from "./image.ts";
import * as Locale from "./locale.ts";
import * as Origin from "./origin.ts";
import * as SafeArea from "./safe-area.ts";
import * as Sensor from "./sensor.ts";
import * as Storage from "./storage.ts";
import * as Surface from "./surface.tsx";
import * as Text from "./text.ts";
import * as VisualViewport from "./visual-viewport.ts";

export const CONFORMS = [
  AboveKeyboard satisfies typeof WebAboveKeyboard,
  Core satisfies typeof WebCore,
  DevOverlay satisfies typeof WebDevOverlay,
  Glass satisfies typeof WebGlass,
  Gpu satisfies typeof WebGpu,
  Host satisfies typeof WebHost,
  Image satisfies typeof WebImage,
  Locale satisfies typeof WebLocale,
  Origin satisfies typeof WebOrigin,
  SafeArea satisfies typeof WebSafeArea,
  Sensor satisfies typeof WebSensor,
  Storage satisfies typeof WebStorage,
  Surface satisfies typeof WebSurface,
  Text satisfies typeof WebText,
  VisualViewport satisfies typeof WebVisualViewport,
];
