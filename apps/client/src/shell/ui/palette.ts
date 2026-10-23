// @pxl/client/shell/ui/palette.ts

/** iOS's light-mode label and status colors, so the page and an iPhone draw alike. */

import { alpha, color, SECONDARY, SYSTEM } from "@/shell/ui/theme.ts";

export const LABEL: { primary: string; secondary: string; tertiary: string; placeholder: string } = {
  primary: color.black,
  secondary: alpha(color.labelGray, SECONDARY),
  tertiary: alpha(color.labelGray, 0.3),
  placeholder: alpha(color.labelGray, 0.3),
};

export const NORTH: string = SYSTEM.red;

export const STATUS: { ready: string; pending: string; failed: string } = {
  ready: SYSTEM.green,
  pending: SYSTEM.orange,
  failed: SYSTEM.red,
};
