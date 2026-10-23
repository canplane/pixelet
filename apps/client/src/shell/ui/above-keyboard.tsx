// @pxl/client/shell/ui/above-keyboard.tsx

import { useSafeArea } from "@/bracket.tsx";
import { type AboveKeyboardProps, bottomRest } from "@/shell/ui/bottom-rest.ts";
import { layer } from "@/shell/ui/theme.ts";

/**
 * A bottom layer standing `inset` above the keyboard via `--vv-bottom`, a CSS
 * variable since it follows every frame. iOS streams viewport events or sends
 * one final value, so `bottom` gets a short ease.
 */
export function AboveKeyboard({ inset, lift = 0, children }: AboveKeyboardProps) {
  const rest = bottomRest(useSafeArea(), inset);
  return (
    <div
      style={{
        position: "absolute",
        left: inset,
        right: inset,
        bottom: `calc(max(${rest}px, ${inset}px + var(--vv-bottom, 0px)) + ${lift}px)`,
        zIndex: layer.bottom,
        transition: "bottom 150ms ease-out",
        pointerEvents: "none",
      }}
    >
      {children}
    </div>
  );
}
