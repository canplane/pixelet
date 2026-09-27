// @pxl/web/shared/ui/glass-variant.ts

/** Glass thickness, thinnest first: `None` draws only the fill, `Clear` is
 * for controls on a sheet, `Regular` (frosted) for controls over the map.
 * While fading, a device shows at most `Clear` (the device app's
 * platform/glass.tsx). The web has no glass and always draws the fill. */
export const enum GlassVariant {
  None = 0,
  Clear = 1,
  Regular = 2,
}
