// @pxl/native/platform/backing.tsx

/**
 * A floating control's backing on an iPhone: Liquid Glass where the system
 * has it, else the fill. Presence is never an opacity on the glass: UIKit
 * renders a visual effect wrongly under any alpha below one, or never
 * installs it again. It goes through the glass styles ("clear", "none").
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
  /** With a material, a `#rrggbb` the material sets the opacity of. */
  fill: ColorValue;
  material?: Material;
  tint?: ColorValue;
  interactive?: boolean;
  place: BackingPlace;
  fade?: f64;
  fadeFrom?: f64;
  transition?: Transition;
  /** No transform: that is the fade's. */
  style?: StyleProp<ViewStyle>;
  children?: ReactNode;
}

export const SYSTEM_BACKING = isLiquidGlassAvailable();

/** UIKit blurs by a material, not a radius, so `blur` has no meaning here. */
const Blur: ComponentType<ViewProps> = requireNativeView("PxlPlatform", "Blur");

type Effect = "none" | "clear" | "regular";

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
  // A style set before the first layout is installed without animation, so
  // it waits for layout and the transition's delay.
  const [laidOut, setLaidOut] = useState(false);
  useEffect(() => {
    if (!laidOut || shown === target) return;
    const delay = veil.transition.delayMs ?? 0;
    if (delay === 0) return setShown(target);
    const timer = setTimeout(() => setShown(target), delay);
    return () => clearTimeout(timer);
  }, [target, laidOut]);

  // A see-through backing is the system blur, not the glass, whose rim ran as
  // a line across the sheet. The blur and fill are cut to the corners; the
  // backing is not, which would cut its shadow.
  if (material) {
    const { borderRadius } = StyleSheet.flatten(style) ?? {};
    const round = { borderRadius, overflow: "hidden" as const };
    return (
      <Motion from={from} to={{ opacity: veil.level }} transition={veil.transition} style={style}>
        <Blur style={[StyleSheet.absoluteFill, round]} />
        <View style={[StyleSheet.absoluteFill, round, { backgroundColor: alpha(fill as string, material.system) }]} />
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
  layout: { flexGrow: 1, alignSelf: "stretch" },
});
