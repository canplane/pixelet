// @pxl/native/platform/glass.tsx

/**
 * A floating control's ground, on a device: the system's glass where it has
 * one — Liquid Glass, from iOS 26, which bends and lights what is under it,
 * the map included. Where there is no system glass, or the variant is
 * `None` (shared/ui/glass-variant.ts), it draws the fill instead, as the web
 * does.
 *
 * Its presence (shared/ui/motion/veil.ts) is never an opacity on the glass or
 * above it: UIKit renders a visual effect wrongly under any alpha below one,
 * or skips it and never installs it again. So presence is shown by the
 * glass's thickness instead: the given variant when fully present, at most
 * `Clear` while fading, `None` when gone. The system animates between them.
 * The content on the glass fades by opacity, which is safe.
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
import { SNAP, type Transition } from "@/shared/ui/motion/transition.ts";
import { useVeil } from "@/shared/ui/motion/veil.ts";
import { GlassVariant } from "@/shared/ui/glass-variant.ts";

export interface GlassProps {
  /** The ground where there is no glass. */
  fill: ColorValue;
  /** The glass's tint where there is glass. */
  tint?: ColorValue;
  /** Whether the glass answers a touch, as a control's does. */
  interactive?: boolean;
  /** The glass thickness (shared/ui/glass-variant.ts). */
  variant: GlassVariant;
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

/** expo-glass-effect's style name for each GlassVariant value. */
const EFFECT = ["none", "clear", "regular"] as const;

/** The thickness to show at a presence level: `None` at 0, `whole` at 1,
 * and at most `Clear` in between. */
function thicknessAt(level: f64, whole: GlassVariant): GlassVariant {
  if (level <= 0) return GlassVariant.None;
  return level < 1 && whole === GlassVariant.Regular ? GlassVariant.Clear : whole;
}

export function Glass({ fill, tint, interactive, variant, fade = 1, fadeFrom, transition = SNAP, style, children }: GlassProps) {
  const veil = useVeil(fade, transition);
  const from = fadeFrom === undefined ? undefined : { opacity: fadeFrom };

  const target = thicknessAt(veil.level, variant);
  const [shown, setShown] = useState<GlassVariant>(fadeFrom === undefined ? target : thicknessAt(fadeFrom, variant));
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

  if (!SYSTEM_GLASS || variant === GlassVariant.None) {
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
      glassEffectStyle={{ style: EFFECT[shown], animate: true, animationDuration: veil.transition.durationMs / 1000 }}
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
