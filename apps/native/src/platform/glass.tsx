// @pxl/native/platform/glass.tsx

/**
 * A floating control's ground, on a device: the system's glass where it has
 * one — Liquid Glass, from iOS 26, which bends and lights what is under it,
 * the map included — and the control's fill where it has none, as on the web.
 *
 * Its presence (shared/ui/motion/veil.ts) is never an opacity on the glass or
 * above it: UIKit renders a visual effect wrongly under any alpha below one,
 * or skips it and never installs it again. The glass shows it in its own
 * style instead — whole, a thinner `clear` while only partly there, `none`
 * when gone — which the system animates, materializing it and taking it
 * away; what stands on the glass fades by opacity, which it can.
 *
 * The system glass draws its own rim and depth, so a border or a shadow in
 * the style is the fill's alone: on the glass, it would neither fade with it
 * nor match it. A color it is to carry is its tint.
 */

import { GlassView, isLiquidGlassAvailable } from "expo-glass-effect";
import { type ReactNode, useEffect, useState } from "react";
import { type ColorValue, type StyleProp, StyleSheet, type ViewStyle } from "react-native";

import type { f64 } from "@xpute/core/abi/word.ts";
import { Motion } from "@platform/motion.tsx";
import { durationMs, SNAP, type Transition } from "@/shared/ui/motion/transition.ts";
import { useVeil } from "@/shared/ui/motion/veil.ts";

export interface GlassProps {
  /** The ground where there is no glass. */
  fill: ColorValue;
  /** The glass's tint where there is glass. */
  tint?: ColorValue;
  /** Whether the glass answers a touch, as a control's does. */
  interactive?: boolean;
  /** How present it is, from 1 to 0 (gone); and what it mounts at. */
  fade?: f64;
  fadeFrom?: f64;
  /** How a change of `fade` is moved through. */
  transition?: Transition;
  /** Its box, border and shadow; no transform, which is its fade's. */
  style?: StyleProp<ViewStyle>;
  children?: ReactNode;
}

/** Whether a surface here is the system's glass. Asked once: the OS does
 * not gain or lose glass while the app runs. */
export const SYSTEM_GLASS = isLiquidGlassAvailable();

type GlassStyle = "regular" | "clear" | "none";

function glassAt(level: f64): GlassStyle {
  return level <= 0 ? "none" : level < 1 ? "clear" : "regular";
}

export function Glass({ fill, tint, interactive, fade = 1, fadeFrom, transition = SNAP, style, children }: GlassProps) {
  const veil = useVeil(fade, transition);
  const from = fadeFrom === undefined ? undefined : { opacity: fadeFrom };

  const target = glassAt(veil.level);
  const [shown, setShown] = useState<GlassStyle>(fadeFrom === undefined ? target : glassAt(fadeFrom));
  // A style is applied only once the glass has been laid out: the native view
  // installs its effect in its first layout, and a style set before that is
  // installed as it is, without the animation: glass meant to materialize
  // would stand whole from its first frame. And after the transition's
  // delay, which the glass has none of its own for.
  const [laidOut, setLaidOut] = useState(false);
  useEffect(() => {
    if (!laidOut || shown === target) return;
    const delay = veil.transition.delayMs ?? 0;
    if (delay === 0) return setShown(target);
    const timer = setTimeout(() => setShown(target), delay);
    return () => clearTimeout(timer);
  }, [target, laidOut]);

  if (!SYSTEM_GLASS) {
    return (
      <Motion from={from} to={{ opacity: veil.level }} transition={veil.transition} style={[style, { backgroundColor: fill }]}>
        {children}
      </Motion>
    );
  }

  const { boxShadow: _shadow, borderWidth: _width, borderColor: _color, flexDirection, alignItems, justifyContent, gap, rowGap, columnGap, ...ground } = StyleSheet.flatten(style) ?? {};
  return (
    <GlassView
      onLayout={laidOut ? undefined : () => setLaidOut(true)}
      style={ground}
      glassEffectStyle={{ style: shown, animate: true, animationDuration: durationMs(veil.transition) / 1000 }}
      tintColor={tint}
      isInteractive={interactive}
    >
      <Motion
        from={from}
        to={{ opacity: veil.level }}
        transition={veil.transition}
        style={[styles.layout, { flexDirection, alignItems, justifyContent, gap, rowGap, columnGap }]}
      >
        {children}
      </Motion>
    </GlassView>
  );
}

const styles = StyleSheet.create({
  // What stands on the glass, laid out as the glass's own style says.
  layout: { flexGrow: 1, alignSelf: "stretch" },
});
