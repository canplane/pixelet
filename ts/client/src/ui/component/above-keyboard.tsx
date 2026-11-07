// @pxl/client/ui/component/above-keyboard.tsx

import {
  type AboveKeyboardProps,
  bottomRest,
} from "@/ui/component/bottom-rest.ts";
import { layer } from "@/ui/component/theme.ts";
import { VISUAL_VIEWPORT_BOTTOM } from "@/service/types.ts";
import { useSafeArea } from "@/service/safe-area.ts";

/**
 * A bottom layer standing `inset` above the keyboard via the page's
 * `VISUAL_VIEWPORT_BOTTOM` (service/viewport.ts), a CSS variable since it follows every frame. iOS
 * streams viewport events or sends one final value, so `bottom` gets a short
 * ease.
 */
export function AboveKeyboard({
  inset,
  lift = 0,
  children,
}: AboveKeyboardProps) {
  const rest = bottomRest(useSafeArea(), inset);
  return (
    <div
      style={{
        position: "absolute",
        left: inset,
        right: inset,
        bottom: `calc(max(${rest}px, ${inset}px + var(${VISUAL_VIEWPORT_BOTTOM}, 0px)) + ${lift}px)`,
        zIndex: layer.bottom,
        transition: "bottom 150ms ease-out",
        pointerEvents: "none",
      }}
    >
      {children}
    </div>
  );
}
