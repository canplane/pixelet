// @pxl/web/shared/ui/overlay_safe_area.ts

/**
 * Shared vertical "safe area" constants for floating overlay chrome above
 * the 3D canvas.
 *
 * The floating mode bar (bottom-center) is the tallest, most central
 * bottom-anchored UI and is only going to grow (search/text-mode entry
 * points are planned), so every other bottom-anchored piece (toast pill,
 * compass/scale_bar/GPS cluster) needs to clear its footprint.
 * Centralizing that footprint here, instead of each consumer guessing at
 * it independently, keeps them from drifting out of sync as it changes.
 */

/** Mode bar's own `bottom-4` offset from the viewport edge. */
export const MODE_BAR_BOTTOM_OFFSET_PX = 16;
/** Live-measured total height (border-box) of the mode bar pill. */
export const MODE_BAR_HEIGHT_PX = 50;
/** Minimum breathing room between the mode bar's top edge and whatever sits above it. */
export const BOTTOM_SAFE_GAP_PX = 16;

/**
 * Distance from the viewport's bottom edge that any other bottom-anchored
 * overlay element (toast container, compass/scale_bar/GPS cluster, ...)
 * should sit at or beyond, so nothing overlaps the mode bar's own
 * reserved space.
 */
export const BOTTOM_OVERLAY_SAFE_PX = MODE_BAR_BOTTOM_OFFSET_PX + MODE_BAR_HEIGHT_PX + BOTTOM_SAFE_GAP_PX;
