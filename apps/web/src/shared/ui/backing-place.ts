// @pxl/web/shared/ui/backing-place.ts

/** Decides the iPhone's glass style: clear `OnSheet`, regular (frosted)
 * where what is behind moves. A page draws the fill everywhere. */
export const enum BackingPlace {
  OnSheet = 0,
  OverMap = 1,
  OverContent = 2,
}
