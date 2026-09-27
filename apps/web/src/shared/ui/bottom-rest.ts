// @pxl/web/shared/ui/bottom-rest.ts

import type { SafeArea } from "@platform/safe-area.ts";

/**
 * How far off the screen's bottom edge the bottom chrome rests, with no
 * keyboard up: clear of what at the bottom takes a touch, by `inset`, and
 * never nearer the edge than `inset`.
 *
 * What takes a touch there is the system's to say (`tappable`): Android's
 * navigation buttons do, and a tap meant for the chrome must not land on
 * them, so the chrome stands `inset` above them, as Material has a floating
 * element stand above the system bars. What only takes a gesture — an
 * iPhone's home indicator, Android's gesture handle — is edge, not a
 * control, so the chrome sits on it where that is higher than `inset`, as a
 * toolbar sits on the safe area; a margin on top of it would leave the
 * bottom emptier than the sides. A page has neither: the browser keeps its
 * bars off the viewport.
 */
export function bottomRest(area: SafeArea, inset: number): number {
  return area.tappable + Math.max(inset, area.bottom - area.tappable);
}
