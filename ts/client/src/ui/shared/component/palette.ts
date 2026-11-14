// @pxl/client/ui/shared/component/palette.ts

/** iOS's label and status colors as they are in light mode, so that a page
 * and Android draw as an iPhone does. */

import {
  alpha,
  color,
  SECONDARY,
  SYSTEM,
} from "@/ui/shared/component/theme.ts";

export const LABEL: {
  primary: string;
  secondary: string;
  tertiary: string;
  placeholder: string;
} = {
  primary: color.black,
  secondary: alpha(color.label_gray, SECONDARY),
  tertiary: alpha(color.label_gray, 0.3),
  placeholder: alpha(color.label_gray, 0.3),
};

export const NORTH: string = SYSTEM.red;

export const STATUS: { ready: string; pending: string; failed: string } = {
  ready: SYSTEM.green,
  pending: SYSTEM.orange,
  failed: SYSTEM.red,
};
