// @pxl/client/conform.ts

/**
 * Holds every platform file to the default beside it, which a bundler swaps
 * in by the file's suffix (pill.ios.tsx for pill.tsx on an iPhone) and so no
 * import ever names. Nothing imports this; `deno check src` reads it.
 */

import type * as AboveKeyboard from "./shell/ui/above-keyboard.tsx";
import * as AboveKeyboardNative from "./shell/ui/above-keyboard.native.tsx";
import type * as Backing from "./shell/ui/backing.tsx";
import * as BackingIos from "./shell/ui/backing.ios.tsx";
import type * as Icon from "./shell/ui/icon.tsx";
import * as IconIos from "./shell/ui/icon.ios.tsx";
import type * as Motion from "./shell/ui/motion/motion.tsx";
import * as MotionNative from "./shell/ui/motion/motion.native.tsx";
import type * as Palette from "./shell/ui/palette.ts";
import * as PaletteIos from "./shell/ui/palette.ios.ts";
import type * as Panel from "./shell/ui/panel.tsx";
import * as PanelAndroid from "./shell/ui/panel.android.tsx";
import * as PanelIos from "./shell/ui/panel.ios.tsx";
import type * as Pill from "./shell/ui/pill.tsx";
import * as PillIos from "./shell/ui/pill.ios.tsx";
import type * as Segmented from "./shell/ui/segmented.tsx";
import * as SegmentedIos from "./shell/ui/segmented.ios.tsx";
import type * as ToggleRow from "./shell/ui/toggle-row.tsx";
import * as ToggleRowIos from "./shell/ui/toggle-row.ios.tsx";

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
