// @pxl/client/ui/shared/component/palette.ts

/** iOS's label and status colors as they are in light mode, so that the web
 * and Android draw as an iPhone does. */

import {
  alpha,
  COLOR,
  SECONDARY,
  SYSTEM,
} from "@/ui/shared/component/theme.ts";

export const LABEL: {
  primary: string;
  secondary: string;
  tertiary: string;
  placeholder: string;
} = {
  primary: COLOR.black,
  secondary: alpha(COLOR.labelGray, SECONDARY),
  tertiary: alpha(COLOR.labelGray, 0.3),
  placeholder: alpha(COLOR.labelGray, 0.3),
};

export const STATUS: { ready: string; pending: string; failed: string } = {
  ready: SYSTEM.green,
  pending: SYSTEM.orange,
  failed: SYSTEM.red,
};
