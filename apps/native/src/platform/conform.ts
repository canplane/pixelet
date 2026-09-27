// @pxl/native/platform/conform.ts

/**
 * The device's platform held to the web's: every module here exports what
 * the web's does, of the type the web's has. The shell is type-checked with
 * `@platform/` as the web's (apps/web/deno.jsonc), so a device's module that
 * lacked a name, or took another shape, would pass the check and fail only
 * when Metro ran it; root deno.jsonc's check:native checks this file and fails
 * there instead. A module may export more than the web's does, never less.
 * A module's Android file beside it — `panel.android.tsx` beside `panel.tsx`
 * — is what Metro takes for Android (metro.config.js); it is held to the
 * web's the same way.
 *
 * Nothing imports this. It is a check, not code, and Metro never sees it.
 */

import type * as WebAboveKeyboard from "../../../web/src/platform/web/above-keyboard.tsx";
import type * as WebCore from "../../../web/src/platform/web/core.ts";
import type * as WebBacking from "../../../web/src/platform/web/backing.tsx";
import type * as WebBackdrop from "../../../web/src/platform/web/backdrop.ts";
import type * as WebIcon from "../../../web/src/platform/web/icon.tsx";
import type * as WebGesture from "../../../web/src/platform/web/gesture.ts";
import type * as WebGpu from "../../../web/src/platform/web/gpu.ts";
import type * as WebImage from "../../../web/src/platform/web/image.ts";
import type * as WebLocale from "../../../web/src/platform/web/locale.ts";
import type * as WebMotion from "../../../web/src/platform/web/motion.tsx";
import type * as WebOrigin from "../../../web/src/platform/web/origin.ts";
import type * as WebPalette from "../../../web/src/platform/web/palette.ts";
import type * as WebPanel from "../../../web/src/platform/web/panel.tsx";
import type * as WebPill from "../../../web/src/platform/web/pill.tsx";
import type * as WebSafeArea from "../../../web/src/platform/web/safe-area.ts";
import type * as WebSave from "../../../web/src/platform/web/save.ts";
import type * as WebSegmented from "../../../web/src/platform/web/segmented.tsx";
import type * as WebSensor from "../../../web/src/platform/web/sensor.ts";
import type * as WebStorage from "../../../web/src/platform/web/storage.ts";
import type * as WebSurface from "../../../web/src/platform/web/surface.tsx";
import type * as WebText from "../../../web/src/platform/web/text.ts";
import type * as WebToggleRow from "../../../web/src/platform/web/toggle-row.tsx";
import type * as WebTextField from "../../../web/src/platform/web/text-field.ts";
import type * as WebTypeface from "../../../web/src/platform/web/typeface.ts";
import type * as WebVisualViewport from "../../../web/src/platform/web/viewport.ts";

import * as AboveKeyboard from "./above-keyboard.tsx";
import * as Core from "./core.ts";
import * as Backing from "./backing.tsx";
import * as Backdrop from "./backdrop.ts";
import * as Icon from "./icon.tsx";
import * as Gesture from "./gesture.ts";
import * as Gpu from "./gpu.ts";
import * as Image from "./image.ts";
import * as Locale from "./locale.ts";
import * as Motion from "./motion.tsx";
import * as Origin from "./origin.ts";
import * as Palette from "./palette.ts";
import * as Panel from "./panel.tsx";
import * as Pill from "./pill.tsx";
import * as SafeArea from "./safe-area.ts";
import * as Save from "./save.ts";
import * as Segmented from "./segmented.tsx";
import * as Sensor from "./sensor.ts";
import * as Storage from "./storage.ts";
import * as Surface from "./surface.tsx";
import * as Text from "./text.ts";
import * as ToggleRow from "./toggle-row.tsx";
import * as TextField from "./text-field.ts";
import * as Typeface from "./typeface.ts";
import * as VisualViewport from "./viewport.ts";
import * as BackingAndroid from "./backing.android.tsx";
import * as IconAndroid from "./icon.android.tsx";
import * as PaletteAndroid from "./palette.android.ts";
import * as PanelAndroid from "./panel.android.tsx";
import * as PillAndroid from "./pill.android.tsx";
import * as SegmentedAndroid from "./segmented.android.tsx";
import * as ToggleRowAndroid from "./toggle-row.android.tsx";
import * as TextFieldAndroid from "./text-field.android.ts";

export const CONFORMS = [
  AboveKeyboard satisfies typeof WebAboveKeyboard,
  Core satisfies typeof WebCore,
  Backing satisfies typeof WebBacking,
  Backdrop satisfies typeof WebBackdrop,
  Icon satisfies typeof WebIcon,
  Gesture satisfies typeof WebGesture,
  Gpu satisfies typeof WebGpu,
  Image satisfies typeof WebImage,
  Locale satisfies typeof WebLocale,
  Motion satisfies typeof WebMotion,
  Origin satisfies typeof WebOrigin,
  Palette satisfies typeof WebPalette,
  Panel satisfies typeof WebPanel,
  Pill satisfies typeof WebPill,
  SafeArea satisfies typeof WebSafeArea,
  Save satisfies typeof WebSave,
  Segmented satisfies typeof WebSegmented,
  Sensor satisfies typeof WebSensor,
  Storage satisfies typeof WebStorage,
  Surface satisfies typeof WebSurface,
  Text satisfies typeof WebText,
  ToggleRow satisfies typeof WebToggleRow,
  TextField satisfies typeof WebTextField,
  Typeface satisfies typeof WebTypeface,
  VisualViewport satisfies typeof WebVisualViewport,
  BackingAndroid satisfies typeof WebBacking,
  IconAndroid satisfies typeof WebIcon,
  PaletteAndroid satisfies typeof WebPalette,
  PanelAndroid satisfies typeof WebPanel,
  PillAndroid satisfies typeof WebPill,
  SegmentedAndroid satisfies typeof WebSegmented,
  ToggleRowAndroid satisfies typeof WebToggleRow,
  TextFieldAndroid satisfies typeof WebTextField,
];
