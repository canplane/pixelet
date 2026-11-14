// @pxl/client/conform.ts

/**
 * Makes `tsc` hold every platform file to the default file beside it: a
 * device's file (`x.native.ts`) to the page's, and a piece of the UI's
 * override (`x.ios.tsx`, `x.android.tsx`, `x.native.tsx`) to its default. The
 * bundler swaps a platform file in by its suffix, so no import names it, and
 * without this file nothing would check that it exports what the default
 * does. Nothing imports this file; `tsc` alone reads it.
 */

import type * as page from "@/page.ts";
import * as page_native from "@/page.native.ts";
import type * as boot from "@/runtime/boot.ts";
import * as boot_native from "@/runtime/boot.native.ts";
import type * as db from "@/service/platform/db.ts";
import * as db_native from "@/service/platform/db.native.ts";
import type * as device from "@/service/platform/device.ts";
import * as device_native from "@/service/platform/device.native.ts";
import type * as locale from "@/service/platform/locale.ts";
import * as locale_native from "@/service/platform/locale.native.ts";
import type * as log from "@/service/platform/log.ts";
import * as log_native from "@/service/platform/log.native.ts";
import type * as online from "@/service/platform/online.ts";
import * as online_native from "@/service/platform/online.native.ts";
import type * as origin from "@/service/platform/origin.ts";
import * as origin_native from "@/service/platform/origin.native.ts";
import type * as safe_area from "@/service/platform/safe-area.ts";
import * as safe_area_native from "@/service/platform/safe-area.native.ts";
import type * as sensor from "@/service/platform/sensor.ts";
import * as sensor_native from "@/service/platform/sensor.native.ts";
import type * as share from "@/service/platform/share.ts";
import * as share_native from "@/service/platform/share.native.ts";
import type * as storage from "@/service/platform/storage.ts";
import * as storage_native from "@/service/platform/storage.native.ts";
import type * as viewport from "@/service/platform/viewport.ts";
import * as viewport_native from "@/service/platform/viewport.native.ts";
import type * as visibility from "@/service/platform/visibility.ts";
import * as visibility_native from "@/service/platform/visibility.native.ts";
import type * as surface from "@/service/platform/surface.tsx";
import * as surface_native from "@/service/platform/surface.native.tsx";
import type * as above_keyboard from "@/ui/shared/component/above-keyboard.tsx";
import * as above_keyboard_native from "@/ui/shared/component/above-keyboard.native.tsx";
import type * as backdrop from "@/ui/shared/component/backdrop.ts";
import * as backdrop_native from "@/ui/shared/component/backdrop.native.ts";
import type * as backing from "@/ui/shared/component/backing.tsx";
import * as backing_ios from "@/ui/shared/component/backing.ios.tsx";
import type * as icon from "@/ui/shared/component/icon.tsx";
import * as icon_ios from "@/ui/shared/component/icon.ios.tsx";
import type * as motion from "@/ui/shared/component/motion/motion.tsx";
import * as motion_native from "@/ui/shared/component/motion/motion.native.tsx";
import type * as palette from "@/ui/shared/component/palette.ts";
import * as palette_ios from "@/ui/shared/component/palette.ios.ts";
import type * as pan from "@/ui/shared/component/pan.ts";
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
import type * as typeface from "@/ui/shared/component/typeface.ts";
import * as typeface_native from "@/ui/shared/component/typeface.native.ts";

export const CONFORMS = [
  page_native satisfies typeof page,
  boot_native satisfies typeof boot,
  db_native satisfies typeof db,
  device_native satisfies typeof device,
  locale_native satisfies typeof locale,
  log_native satisfies typeof log,
  online_native satisfies typeof online,
  origin_native satisfies typeof origin,
  safe_area_native satisfies typeof safe_area,
  sensor_native satisfies typeof sensor,
  share_native satisfies typeof share,
  storage_native satisfies typeof storage,
  viewport_native satisfies typeof viewport,
  visibility_native satisfies typeof visibility,
  surface_native satisfies typeof surface,
  above_keyboard_native satisfies typeof above_keyboard,
  backdrop_native satisfies typeof backdrop,
  backing_ios satisfies typeof backing,
  icon_ios satisfies typeof icon,
  motion_native satisfies typeof motion,
  palette_ios satisfies typeof palette,
  pan_native satisfies typeof pan,
  panel_android satisfies typeof panel,
  panel_ios satisfies typeof panel,
  pill_ios satisfies typeof pill,
  segmented_ios satisfies typeof segmented,
  text_field_ios satisfies typeof text_field,
  toggle_row_ios satisfies typeof toggle_row,
  typeface_native satisfies typeof typeface,
];
