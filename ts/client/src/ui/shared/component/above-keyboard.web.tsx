// @pxl/client/ui/shared/component/above-keyboard.web.tsx

import {
  type AboveKeyboardProps,
  bottom_rest,
} from "@/ui/shared/component/bottom-rest.ts";
import { LAYER } from "@/ui/shared/component/theme.ts";
import { VISUAL_VIEWPORT_BOTTOM } from "@/service/platform/types.ts";
import { useSafeArea } from "@/service/platform/safe-area.ts";

/**
 * The layer stands `inset` above the keyboard. Where the keyboard's top is
 * comes from the CSS variable `VISUAL_VIEWPORT_BOTTOM`, which
 * service/platform/viewport.web.ts sets, rather than from React state,
 * because it changes on every frame. iOS either streams viewport events or
 * sends only the final value, so `bottom` eases briefly to smooth the jump in
 * the second case.
 */
export function AboveKeyboard({
  inset,
  lift = 0,
  children,
}: AboveKeyboardProps) {
  const rest = bottom_rest(useSafeArea(), inset);
  return (
    <div
      style={{
        position: "absolute",
        left: inset,
        right: inset,
        bottom: `calc(max(${rest}px, ${inset}px + var(${VISUAL_VIEWPORT_BOTTOM}, 0px)) + ${lift}px)`,
        zIndex: LAYER.bottom,
        transition: "bottom 150ms ease-out",
        pointerEvents: "none",
      }}
    >
      {children}
    </div>
  );
}
