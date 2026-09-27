// @pxl/web/shared/ui/backing-place.ts

/** Where a floating control's backing stands, which is what it is drawn as
 * where the system draws it: `OnSheet` on a sheet's plain ground, which an
 * iPhone draws as clear glass; `OverMap` over the world, and `OverContent`
 * over a sheet's content that scrolls under it, both drawn as its regular,
 * frosted glass, since what is behind them moves. A page draws the fill
 * wherever it stands. */
export const enum BackingPlace {
  OnSheet = 0,
  OverMap = 1,
  OverContent = 2,
}
