// @pxl/client/ui/component/conform.ts

/**
 * Makes `tsc` hold every platform file to the default file beside it. The
 * bundler swaps a platform file in by its suffix (pill.ios.tsx for pill.tsx on
 * an iPhone), so no import names it, and without this file nothing would
 * check that it exports what the default does. Nothing imports this file;
 * `tsc` alone reads it.
 */

import type * as AboveKeyboard from "./above-keyboard.tsx";
import * as AboveKeyboardNative from "./above-keyboard.native.tsx";
import type * as Backing from "./backing.tsx";
import * as BackingIos from "./backing.ios.tsx";
import type * as Icon from "./icon.tsx";
import * as IconIos from "./icon.ios.tsx";
import type * as Motion from "@/ui/component/motion/motion.tsx";
import * as MotionNative from "@/ui/component/motion/motion.native.tsx";
import type * as Palette from "./palette.ts";
import * as PaletteIos from "./palette.ios.ts";
import type * as Panel from "./panel.tsx";
import * as PanelAndroid from "./panel.android.tsx";
import * as PanelIos from "./panel.ios.tsx";
import type * as Pill from "@/ui/world/pill.tsx";
import * as PillIos from "@/ui/world/pill.ios.tsx";
import type * as Segmented from "./segmented.tsx";
import * as SegmentedIos from "./segmented.ios.tsx";
import type * as ToggleRow from "@/ui/setting/toggle-row.tsx";
import * as ToggleRowIos from "@/ui/setting/toggle-row.ios.tsx";

export const CONFORMS = [
  AboveKeyboardNative satisfies typeof AboveKeyboard,
  BackingIos satisfies typeof Backing,
  IconIos satisfies typeof Icon,
  MotionNative satisfies typeof Motion,
  PaletteIos satisfies typeof Palette,
  PanelAndroid satisfies typeof Panel,
  PanelIos satisfies typeof Panel,
  PillIos satisfies typeof Pill,
  SegmentedIos satisfies typeof Segmented,
  ToggleRowIos satisfies typeof ToggleRow,
];
