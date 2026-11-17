// @pxl/client/conform.ts

/**
 * Makes `tsc` hold every platform file to the file beside it: the native file
 * (`x.native.ts`) to the web one (`x.web.ts`), and a piece of the UI's override
 * (`x.ios.tsx`, `x.android.tsx`) to its default. The bundler swaps a platform
 * file in by its suffix, so no import names it, and without this file nothing
 * would check that it exports what the other does. A file that has no twin
 * and is imported by its name, a native module of its own
 * (service/platform/pxl-platform.native.ts) and the web text
 * (service/platform/text.web.ts), is none of the files this one holds.
 * Nothing imports this file; `tsc` alone reads it.
 */

import type * as pageWeb from "@/page.web.ts";
import * as pageNative from "@/page.native.ts";
import type * as bootWeb from "@/runtime/boot.web.ts";
import * as bootNative from "@/runtime/boot.native.ts";
import type * as dbWeb from "@/service/platform/db.web.ts";
import * as dbNative from "@/service/platform/db.native.ts";
import type * as deviceWeb from "@/service/platform/device.web.ts";
import * as deviceNative from "@/service/platform/device.native.ts";
import type * as driveWeb from "@/service/platform/drive.web.ts";
import * as driveNative from "@/service/platform/drive.native.ts";
import type * as localeWeb from "@/service/platform/locale.web.ts";
import * as localeNative from "@/service/platform/locale.native.ts";
import type * as onlineWeb from "@/service/platform/online.web.ts";
import * as onlineNative from "@/service/platform/online.native.ts";
import type * as originWeb from "@/service/platform/origin.web.ts";
import * as originNative from "@/service/platform/origin.native.ts";
import type * as safeAreaWeb from "@/service/platform/safe-area.web.ts";
import * as safeAreaNative from "@/service/platform/safe-area.native.ts";
import type * as sensorWeb from "@/service/platform/sensor.web.ts";
import * as sensorNative from "@/service/platform/sensor.native.ts";
import type * as shareWeb from "@/service/platform/share.web.ts";
import * as shareNative from "@/service/platform/share.native.ts";
import type * as storageWeb from "@/service/platform/storage.web.ts";
import * as storageNative from "@/service/platform/storage.native.ts";
import type * as viewportWeb from "@/service/platform/viewport.web.ts";
import * as viewportNative from "@/service/platform/viewport.native.ts";
import type * as visibilityWeb from "@/service/platform/visibility.web.ts";
import * as visibilityNative from "@/service/platform/visibility.native.ts";
import type * as surfaceWeb from "@/service/platform/surface.web.tsx";
import * as surfaceNative from "@/service/platform/surface.native.tsx";
import type * as aboveKeyboardWeb from "@/ui/shared/component/above-keyboard.web.tsx";
import * as aboveKeyboardNative from "@/ui/shared/component/above-keyboard.native.tsx";
import type * as backdropWeb from "@/ui/shared/component/backdrop.web.ts";
import * as backdropNative from "@/ui/shared/component/backdrop.native.ts";
import type * as backing from "@/ui/shared/component/backing.tsx";
import * as backingIos from "@/ui/shared/component/backing.ios.tsx";
import type * as icon from "@/ui/shared/component/icon.tsx";
import * as iconIos from "@/ui/shared/component/icon.ios.tsx";
import type * as palette from "@/ui/shared/component/palette.ts";
import * as paletteIos from "@/ui/shared/component/palette.ios.ts";
import type * as panel from "@/ui/shared/component/panel.tsx";
import * as panelAndroid from "@/ui/shared/component/panel.android.tsx";
import * as panelIos from "@/ui/shared/component/panel.ios.tsx";
import type * as pill from "@/ui/feature/world/pill.tsx";
import * as pillIos from "@/ui/feature/world/pill.ios.tsx";
import type * as segmented from "@/ui/shared/component/segmented.tsx";
import * as segmentedIos from "@/ui/shared/component/segmented.ios.tsx";
import type * as textField from "@/ui/shared/component/text-field.tsx";
import * as textFieldIos from "@/ui/shared/component/text-field.ios.tsx";
import type * as toggleRow from "@/ui/feature/setting/toggle-row.tsx";
import * as toggleRowIos from "@/ui/feature/setting/toggle-row.ios.tsx";
import type * as typefaceWeb from "@/ui/shared/component/typeface.web.ts";
import * as typefaceNative from "@/ui/shared/component/typeface.native.ts";

export const CONFORMS = [
  pageNative satisfies typeof pageWeb,
  bootNative satisfies typeof bootWeb,
  dbNative satisfies typeof dbWeb,
  deviceNative satisfies typeof deviceWeb,
  driveNative satisfies typeof driveWeb,
  localeNative satisfies typeof localeWeb,
  onlineNative satisfies typeof onlineWeb,
  originNative satisfies typeof originWeb,
  safeAreaNative satisfies typeof safeAreaWeb,
  sensorNative satisfies typeof sensorWeb,
  shareNative satisfies typeof shareWeb,
  storageNative satisfies typeof storageWeb,
  viewportNative satisfies typeof viewportWeb,
  visibilityNative satisfies typeof visibilityWeb,
  surfaceNative satisfies typeof surfaceWeb,
  aboveKeyboardNative satisfies typeof aboveKeyboardWeb,
  backdropNative satisfies typeof backdropWeb,
  backingIos satisfies typeof backing,
  iconIos satisfies typeof icon,
  paletteIos satisfies typeof palette,
  panelAndroid satisfies typeof panel,
  panelIos satisfies typeof panel,
  pillIos satisfies typeof pill,
  segmentedIos satisfies typeof segmented,
  textFieldIos satisfies typeof textField,
  toggleRowIos satisfies typeof toggleRow,
  typefaceNative satisfies typeof typefaceWeb,
];
