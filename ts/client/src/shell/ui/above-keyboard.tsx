// @pxl/client/shell/ui/above-keyboard.tsx

import { type AboveKeyboardProps, bottomRest } from "@/shell/ui/bottom-rest.ts";
import { layer } from "@/shell/ui/theme.ts";
import { bracket, VISUAL_VIEWPORT_BOTTOM } from "@/bracket.tsx";

/**
 * A bottom layer standing `inset` above the keyboard via the bracket's
 * `VISUAL_VIEWPORT_BOTTOM`, a CSS variable since it follows every frame. iOS
 * streams viewport events or sends one final value, so `bottom` gets a short
 * ease.
 */
export function AboveKeyboard({
  inset,
  lift = 0,
  children,
}: AboveKeyboardProps) {
  const rest = bottomRest(bracket.view.useSafeArea(), inset);
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
