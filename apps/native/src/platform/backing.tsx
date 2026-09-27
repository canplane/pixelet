// @pxl/native/platform/backing.tsx

/**
 * What a floating control stands on — its backing — on an iPhone: the
 * system's glass, Liquid Glass from iOS 26, which bends and lights what is
 * under it, the map included. Where the system has none it draws the fill,
 * as the page does (platform/web/backing.tsx, which Android draws).
 *
 * Its presence (shared/ui/motion/veil.ts) is never an opacity on the glass or
 * above it: UIKit renders a visual effect wrongly under any alpha below one,
 * or skips it and never installs it again. So presence is shown by the
 * glass style instead: its place's style when fully present, "clear"
 * while fading, "none" when gone. The system animates between them.
 * The content on the glass fades by opacity, which is safe.
 *
 * The system glass draws its own rim and depth, so a border or a shadow in
 * the style is the fill's alone: on the glass, it would neither fade with it
 * nor match it. A color it is to carry is its tint.
 */

import { requireNativeView } from "expo";
import { GlassView, isLiquidGlassAvailable } from "expo-glass-effect";
import { type ComponentType, type ReactNode, useEffect, useState } from "react";
import { type ColorValue, type StyleProp, StyleSheet, View, type ViewProps, type ViewStyle } from "react-native";

import type { f64 } from "@xpute/core/abi/word.ts";
import { Motion } from "@platform/motion.tsx";
import { SNAP, type Transition } from "@/shared/ui/motion/transition.ts";
import { useVeil } from "@/shared/ui/motion/veil.ts";
import { BackingPlace } from "@/shared/ui/backing-place.ts";
import { alpha, type Material } from "@/shared/ui/theme.ts";

export interface BackingProps {
  /** The backing where the system draws none; with a material, its color
   * as a `#rrggbb`, which the material sets the opacity of. */
  fill: ColorValue;
  /** Where the system draws none, see-through, as this recipe says; the
   * glass is see-through of its own. */
  material?: Material;
  /** The glass's tint. */
  tint?: ColorValue;
  /** Whether the glass answers a touch, as a control's does. */
  interactive?: boolean;
  /** Where it stands (shared/ui/backing-place.ts): clear glass on a sheet's
   * plain ground, regular over the map or over content that scrolls. */
  place: BackingPlace;
  /** How present it is, from 1 to 0 (gone); and what it mounts at. */
  fade?: f64;
  fadeFrom?: f64;
  /** How a change of `fade` is moved through. */
  transition?: Transition;
  /** Its box, border and shadow; no transform, which is its fade's. */
  style?: StyleProp<ViewStyle>;
  children?: ReactNode;
}

/** Whether the backing here is the system's glass. Asked once: the OS does
 * not gain or lose glass while the app runs. */
export const SYSTEM_BACKING = isLiquidGlassAvailable();

/** The system's blur a bar stands on (modules/platform/ios/BarBlur.swift).
 * UIKit blurs by a material, not a radius, so a material's `blur` has no
 * meaning here. */
const BarBlur: ComponentType<ViewProps> = requireNativeView("PxlPlatform", "BarBlur");

type Effect = "none" | "clear" | "regular";

/** expo-glass-effect's style for a place at a presence level: "none" at 0,
 * "clear" while fading, the place's own style at 1. */
function effectAt(level: f64, place: BackingPlace): Effect {
  if (level <= 0) return "none";
  if (level < 1 || place === BackingPlace.OnSheet) return "clear";
  return "regular";
}

export function Backing({ fill, material, tint, interactive, place, fade = 1, fadeFrom, transition = SNAP, style, children }: BackingProps) {
  const veil = useVeil(fade, transition);
  const from = fadeFrom === undefined ? undefined : { opacity: fadeFrom };

  const target = effectAt(veil.level, place);
  const [shown, setShown] = useState<Effect>(fadeFrom === undefined ? target : effectAt(fadeFrom, place));
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

  // A material that is not the glass, or any material where there is no
  // glass: the system's blur, which every iPhone has, under the fill at the
  // material's `system` opacity. The blur's own tint is mostly hidden by the
  // ground over it, so at rest on the sheet it is the sheet's ground.
  if (material && (!material.glass || !SYSTEM_BACKING)) {
    return (
      <Motion from={from} to={{ opacity: veil.level }} transition={veil.transition} style={style}>
        <BarBlur style={StyleSheet.absoluteFill} />
        <View style={[StyleSheet.absoluteFill, { backgroundColor: alpha(fill as string, material.system) }]} />
        {children}
      </Motion>
    );
  }

  if (!SYSTEM_BACKING) {
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
      glassEffectStyle={{ style: shown, animate: true, animationDuration: veil.transition.durationMs / 1000 }}
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
