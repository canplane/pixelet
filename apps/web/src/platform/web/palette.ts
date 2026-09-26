// @pxl/web/platform/web/palette.ts

/**
 * The chrome's words and glyphs by what they are, not by a color: the text
 * or glyph a control is, what only hints beside it, and the three states a
 * sensor or a report can be in. On the page, the theme's bark-brown ink at
 * two strengths and its own status hues. A device names its system's colors
 * here instead (iOS's label colors), which follow its dark mode and its
 * contrast settings.
 *
 * Two strengths and no third: what is only partly there — loading, disabled
 * — is faded as a presence (shared/ui/motion/veil.ts), not drawn paler.
 */

import { alpha, color } from "@/shared/ui/theme.ts";

export const LABEL: { primary: string; secondary: string; placeholder: string } = {
  primary: color.ink,
  secondary: alpha(color.ink, 0.5),
  placeholder: alpha(color.ink, 0.4),
};

export const STATUS: { ready: string; pending: string; failed: string } = {
  ready: color.ready,
  pending: color.pending,
  failed: color.alarm,
};
