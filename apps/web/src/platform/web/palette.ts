// @pxl/web/platform/web/palette.ts

/**
 * The chrome's words and glyphs by what they are, not by a color: the text
 * or glyph a control is, what only hints beside it, a hairline printed on a
 * surface, a compass's north, and the three states a sensor or a report can
 * be in. On the page, iOS's own in light mode, so the page and an iPhone
 * draw alike. A device names its system's colors here instead, which follow
 * its contrast settings.
 */

import { alpha, color } from "@/shared/ui/theme.ts";

export const LABEL: { primary: string; secondary: string; tertiary: string; placeholder: string } = {
  primary: color.black,
  secondary: alpha(color.labelGray, 0.6),
  tertiary: alpha(color.labelGray, 0.3),
  placeholder: alpha(color.labelGray, 0.3),
};

export const NORTH: string = color.north;

export const STATUS: { ready: string; pending: string; failed: string } = {
  ready: color.ready,
  pending: color.pending,
  failed: color.alarm,
};
