// @pxl/client/shell/ui/backing.ios.tsx

/**
 * A floating control's backing on an iPhone: Liquid Glass where the system
 * has it, else the fill. Presence is never an opacity on the glass: UIKit
 * renders a visual effect wrongly under any alpha below one, or never
 * installs it again. It goes through the glass styles ("clear", "none").
 */

import { requireNativeView } from "expo";
import { GlassView, isLiquidGlassAvailable } from "expo-glass-effect";
import { type ComponentType, useEffect, useState } from "react";
import { StyleSheet, View, type ViewProps } from "react-native";

import type { f64 } from "@xpute/kit/abi/word.ts";
import { Motion } from "@/shell/ui/motion/motion.tsx";
import { SNAP } from "@/shell/ui/motion/transition.ts";
import { useVeil } from "@/shell/ui/motion/veil.ts";
import { BackingPlace, type BackingProps } from "@/shell/ui/backing.tsx";
import { alpha } from "@/shell/ui/theme.ts";
import { Backing as Fill } from "./backing.tsx";

// Everything the default has, but what this file draws itself.
export * from "./backing.tsx";

export const SYSTEM_BACKING = isLiquidGlassAvailable();

/** UIKit blurs by a material, not a radius, so `blur` has no meaning here. */
const Blur: ComponentType<ViewProps> = requireNativeView("PxlPlatform", "Blur");

type Effect = "none" | "clear" | "regular";

function effectAt(level: f64, place: BackingPlace): Effect {
  if (level <= 0) return "none";
  if (level < 1 || place === BackingPlace.OnSheet) return "clear";
  return "regular";
}

export function Backing(
  {
    fill,
    material,
    tint,
    interactive,
    place,
    fade = 1,
    fadeFrom,
    transition = SNAP,
    style,
    children,
  }: BackingProps,
) {
  const veil = useVeil(fade, transition, fadeFrom);

  const target = effectAt(veil.level, place);
  const [shown, setShown] = useState<Effect>(
    fadeFrom === undefined ? target : effectAt(fadeFrom, place),
  );
  // A style set before the first layout is installed without animation, so
  // it waits for layout and the transition's delay.
  const [laidOut, setLaidOut] = useState(false);
  useEffect(() => {
    if (!laidOut || shown === target) return;
    const delay = veil.motion.transition.delayMs ?? 0;
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
      <Motion {...veil.motion} style={style}>
        <Blur style={[StyleSheet.absoluteFill, round]} />
        <View
          style={[StyleSheet.absoluteFill, round, {
            backgroundColor: alpha(fill as string, material.system),
          }]}
        />
        {children}
      </Motion>
    );
  }

  if (!SYSTEM_BACKING) {
    return (
      <Fill
        fill={fill}
        place={place}
        fade={fade}
        fadeFrom={fadeFrom}
        transition={transition}
        style={style}
      >
        {children}
      </Fill>
    );
  }

  const {
    boxShadow: _shadow,
    borderWidth: _width,
    borderColor: _color,
    flexDirection,
    alignItems,
    justifyContent,
    gap,
    rowGap,
    columnGap,
    ...ground
  } = StyleSheet.flatten(style) ?? {};
  return (
    <GlassView
      onLayout={laidOut ? undefined : () => setLaidOut(true)}
      style={ground}
      glassEffectStyle={{
        style: shown,
        animate: true,
        animationDuration: veil.motion.transition.durationMs / 1000,
      }}
      tintColor={tint}
      isInteractive={interactive}
    >
      <Motion
        {...veil.motion}
        style={[styles.layout, {
          flexDirection,
          alignItems,
          justifyContent,
          gap,
          rowGap,
          columnGap,
        }]}
      >
        {children}
      </Motion>
    </GlassView>
  );
}

const styles = StyleSheet.create({
  layout: { flexGrow: 1, alignSelf: "stretch" },
});
