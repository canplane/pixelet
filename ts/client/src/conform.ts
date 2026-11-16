// @pxl/client/conform.ts

/**
 * Makes `tsc` hold every platform file to the file beside it: a device's file
 * (`x.native.ts`) to the page's (`x.web.ts`), and a piece of the UI's override
 * (`x.ios.tsx`, `x.android.tsx`) to its default. The bundler swaps a platform
 * file in by its suffix, so no import names it, and without this file nothing
 * would check that it exports what the other does. A file that has no twin
 * and is imported by its name, a device's own module
 * (service/platform/pxl-platform.native.ts) and a page's text
 * (service/platform/text.web.ts), is none of the files this one holds.
 * Nothing imports this file; `tsc` alone reads it.
 */

import type * as page_web from "@/page.web.ts";
import * as page_native from "@/page.native.ts";
import type * as boot_web from "@/runtime/boot.web.ts";
import * as boot_native from "@/runtime/boot.native.ts";
import type * as db_web from "@/service/platform/db.web.ts";
import * as db_native from "@/service/platform/db.native.ts";
import type * as device_web from "@/service/platform/device.web.ts";
import * as device_native from "@/service/platform/device.native.ts";
import type * as drive_web from "@/service/platform/drive.web.ts";
import * as drive_native from "@/service/platform/drive.native.ts";
import type * as locale_web from "@/service/platform/locale.web.ts";
import * as locale_native from "@/service/platform/locale.native.ts";
import type * as online_web from "@/service/platform/online.web.ts";
import * as online_native from "@/service/platform/online.native.ts";
import type * as origin_web from "@/service/platform/origin.web.ts";
import * as origin_native from "@/service/platform/origin.native.ts";
import type * as safe_area_web from "@/service/platform/safe-area.web.ts";
import * as safe_area_native from "@/service/platform/safe-area.native.ts";
import type * as sensor_web from "@/service/platform/sensor.web.ts";
import * as sensor_native from "@/service/platform/sensor.native.ts";
import type * as share_web from "@/service/platform/share.web.ts";
import * as share_native from "@/service/platform/share.native.ts";
import type * as storage_web from "@/service/platform/storage.web.ts";
import * as storage_native from "@/service/platform/storage.native.ts";
import type * as viewport_web from "@/service/platform/viewport.web.ts";
import * as viewport_native from "@/service/platform/viewport.native.ts";
import type * as visibility_web from "@/service/platform/visibility.web.ts";
import * as visibility_native from "@/service/platform/visibility.native.ts";
import type * as surface_web from "@/service/platform/surface.web.tsx";
import * as surface_native from "@/service/platform/surface.native.tsx";
import type * as above_keyboard_web from "@/ui/shared/component/above-keyboard.web.tsx";
import * as above_keyboard_native from "@/ui/shared/component/above-keyboard.native.tsx";
import type * as backdrop_web from "@/ui/shared/component/backdrop.web.ts";
import * as backdrop_native from "@/ui/shared/component/backdrop.native.ts";
import type * as backing from "@/ui/shared/component/backing.tsx";
import * as backing_ios from "@/ui/shared/component/backing.ios.tsx";
import type * as icon from "@/ui/shared/component/icon.tsx";
import * as icon_ios from "@/ui/shared/component/icon.ios.tsx";
import type * as motion_web from "@/ui/shared/component/motion/motion.web.tsx";
import * as motion_native from "@/ui/shared/component/motion/motion.native.tsx";
import type * as palette from "@/ui/shared/component/palette.ts";
import * as palette_ios from "@/ui/shared/component/palette.ios.ts";
import type * as pan_web from "@/ui/shared/component/pan.web.ts";
import * as pan_native from "@/ui/shared/component/pan.native.ts";
import type * as panel from "@/ui/shared/component/panel.tsx";
import * as panel_android from "@/ui/shared/component/panel.android.tsx";
import * as panel_ios from "@/ui/shared/component/panel.ios.tsx";
import type * as pill from "@/ui/feature/world/pill.tsx";
import * as pill_ios from "@/ui/feature/world/pill.ios.tsx";
import type * as segmented from "@/ui/shared/component/segmented.tsx";
import * as segmented_ios from "@/ui/shared/component/segmented.ios.tsx";
import type * as text_field from "@/ui/shared/component/text-field.tsx";
import * as text_field_ios from "@/ui/shared/component/text-field.ios.tsx";
import type * as toggle_row from "@/ui/feature/setting/toggle-row.tsx";
import * as toggle_row_ios from "@/ui/feature/setting/toggle-row.ios.tsx";
import type * as typeface_web from "@/ui/shared/component/typeface.web.ts";
import * as typeface_native from "@/ui/shared/component/typeface.native.ts";

export const CONFORMS = [
  page_native satisfies typeof page_web,
  boot_native satisfies typeof boot_web,
  db_native satisfies typeof db_web,
  device_native satisfies typeof device_web,
  drive_native satisfies typeof drive_web,
  locale_native satisfies typeof locale_web,
  online_native satisfies typeof online_web,
  origin_native satisfies typeof origin_web,
  safe_area_native satisfies typeof safe_area_web,
  sensor_native satisfies typeof sensor_web,
  share_native satisfies typeof share_web,
  storage_native satisfies typeof storage_web,
  viewport_native satisfies typeof viewport_web,
  visibility_native satisfies typeof visibility_web,
  surface_native satisfies typeof surface_web,
  above_keyboard_native satisfies typeof above_keyboard_web,
  backdrop_native satisfies typeof backdrop_web,
  backing_ios satisfies typeof backing,
  icon_ios satisfies typeof icon,
  motion_native satisfies typeof motion_web,
  palette_ios satisfies typeof palette,
  pan_native satisfies typeof pan_web,
  panel_android satisfies typeof panel,
  panel_ios satisfies typeof panel,
  pill_ios satisfies typeof pill,
  segmented_ios satisfies typeof segmented,
  text_field_ios satisfies typeof text_field,
  toggle_row_ios satisfies typeof toggle_row,
  typeface_native satisfies typeof typeface_web,
];
