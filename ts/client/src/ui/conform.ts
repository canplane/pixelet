// @pxl/client/ui/conform.ts

/**
 * Makes `tsc` hold every platform file to the default file beside it. The
 * bundler swaps a platform file in by its suffix (pill.ios.tsx for pill.tsx on
 * an iPhone), so no import names it, and without this file nothing would
 * check that it exports what the default does. Nothing imports this file;
 * `tsc` alone reads it.
 */

import type * as AboveKeyboard from "@/ui/shared/component/above-keyboard.tsx";
import * as AboveKeyboardNative from "@/ui/shared/component/above-keyboard.native.tsx";
import type * as Backdrop from "@/ui/shared/component/backdrop.ts";
import * as BackdropNative from "@/ui/shared/component/backdrop.native.ts";
import type * as Backing from "@/ui/shared/component/backing.tsx";
import * as BackingIos from "@/ui/shared/component/backing.ios.tsx";
import type * as Icon from "@/ui/shared/component/icon.tsx";
import * as IconIos from "@/ui/shared/component/icon.ios.tsx";
import type * as Motion from "@/ui/shared/component/motion/motion.tsx";
import * as MotionNative from "@/ui/shared/component/motion/motion.native.tsx";
import type * as Palette from "@/ui/shared/component/palette.ts";
import * as PaletteIos from "@/ui/shared/component/palette.ios.ts";
import type * as Pan from "@/ui/shared/component/pan.ts";
import * as PanNative from "@/ui/shared/component/pan.native.ts";
import type * as Panel from "@/ui/shared/component/panel.tsx";
import * as PanelAndroid from "@/ui/shared/component/panel.android.tsx";
import * as PanelIos from "@/ui/shared/component/panel.ios.tsx";
import type * as Pill from "@/ui/feature/world/pill.tsx";
import * as PillIos from "@/ui/feature/world/pill.ios.tsx";
import type * as Segmented from "@/ui/shared/component/segmented.tsx";
import * as SegmentedIos from "@/ui/shared/component/segmented.ios.tsx";
import type * as TextField from "@/ui/shared/component/text-field.tsx";
import * as TextFieldIos from "@/ui/shared/component/text-field.ios.tsx";
import type * as ToggleRow from "@/ui/feature/setting/toggle-row.tsx";
import * as ToggleRowIos from "@/ui/feature/setting/toggle-row.ios.tsx";
import type * as Typeface from "@/ui/shared/component/typeface.ts";
import * as TypefaceNative from "@/ui/shared/component/typeface.native.ts";

export const CONFORMS = [
  AboveKeyboardNative satisfies typeof AboveKeyboard,
  BackdropNative satisfies typeof Backdrop,
  BackingIos satisfies typeof Backing,
  IconIos satisfies typeof Icon,
  MotionNative satisfies typeof Motion,
  PaletteIos satisfies typeof Palette,
  PanNative satisfies typeof Pan,
  PanelAndroid satisfies typeof Panel,
  PanelIos satisfies typeof Panel,
  PillIos satisfies typeof Pill,
  SegmentedIos satisfies typeof Segmented,
  TextFieldIos satisfies typeof TextField,
  ToggleRowIos satisfies typeof ToggleRow,
  TypefaceNative satisfies typeof Typeface,
];
