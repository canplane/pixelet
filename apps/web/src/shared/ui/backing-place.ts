// @pxl/web/shared/ui/backing-place.ts

/** Where a floating control's backing stands, which is what it is drawn as
 * where the system draws it: `OnSheet` on a sheet, which an iPhone draws as
 * clear glass, and `OverMap` over the world, drawn as its regular, frosted
 * glass. A page draws the fill either way. */
export const enum BackingPlace {
  OnSheet = 0,
  OverMap = 1,
}
