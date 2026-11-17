// @pxl/client/ui/shared/component/backing.ios.tsx

/**
 * The backing behind a floating control on an iPhone: Liquid Glass where the
 * system has it, and the plain fill elsewhere. The glass never fades by its
 * opacity, because UIKit renders a visual effect wrongly under any alpha below
 * one, or never installs it again. The glass fades by changing its style
 * instead, through "clear" to "none".
 */

import { requireNativeView } from "expo";
import { GlassView, isLiquidGlassAvailable } from "expo-glass-effect";
import { type ComponentType, useEffect, useState } from "react";
import { StyleSheet, View, type ViewProps } from "react-native";

import type { f64 } from "@xpute/kit/abi/word.ts";
import { Motion } from "@/ui/shared/component/motion/motion.tsx";
import { SNAP } from "@/ui/shared/component/motion/transition.ts";
import { useDim } from "@/ui/shared/component/motion/dim.ts";
import * as BackingPlace from "./backing-place.ts";
import type { BackingProps, BackingPlace as Place } from "./backing.tsx";
import { alpha } from "@/ui/shared/component/theme.ts";
import { Backing as Fill } from "./backing.tsx";

// Every export of the default file, except `SYSTEM_BACKING` and `Backing`: an
// export declared in this file takes the place of a star export with the
// same name.
export * from "./backing.tsx";

export const SYSTEM_BACKING = isLiquidGlassAvailable();

/** The system's blur view (modules/platform/ios/Blur.swift). UIKit blurs by a
 * material, not a radius, so a material's `blur` has no effect here. */
const Blur: ComponentType<ViewProps> = requireNativeView("PxlPlatform", "Blur");

type Effect = "none" | "clear" | "regular";

function effectAt(level: f64, place: Place): Effect {
  if (level <= 0) return "none";
  if (level < 1 || place === BackingPlace.ON_SHEET) return "clear";
  return "regular";
}

export function Backing(props: BackingProps) {
  const {
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
  } = props;
  const dim = useDim(fade, transition, fadeFrom);

  const target = effectAt(dim.level, place);
  const [shown, setShown] = useState<Effect>(
    fadeFrom === undefined ? target : effectAt(fadeFrom, place),
  );
  // UIKit installs a glass style set before the first layout without
  // animating it, so a change of style waits for the first layout and for
  // the transition's delay.
  const [laidOut, setLaidOut] = useState(false);
  useEffect(() => {
    if (!laidOut || shown === target) return;
    const delay = dim.motion.transition.delayMs ?? 0;
    if (delay === 0) return setShown(target);
    const timer = setTimeout(() => setShown(target), delay);
    return () => clearTimeout(timer);
  }, [target, laidOut]);

  // A see-through backing, one with a material, is the system blur rather
  // than the glass, because the glass's rim draws a line across the sheet.
  // The blur and the fill are clipped to the corners. The backing itself is
  // not clipped, because clipping it would cut off its shadow.
  if (material) {
    const { borderRadius } = StyleSheet.flatten(style) ?? {};
    const round = { borderRadius, overflow: "hidden" as const };
    return (
      <Motion {...dim.motion} style={style}>
        <Blur style={[StyleSheet.absoluteFill, round]} />
        <View
          style={[
            StyleSheet.absoluteFill,
            round,
            {
              backgroundColor: alpha(fill as string, material.system),
            },
          ]}
        />
        {children}
      </Motion>
    );
  }

  if (!SYSTEM_BACKING) return <Fill {...props} />;

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
        animationDuration: dim.motion.transition.durationMs / 1000,
      }}
      tintColor={tint}
      isInteractive={interactive}
    >
      <Motion
        {...dim.motion}
        style={[
          STYLE.layout,
          {
            flexDirection,
            alignItems,
            justifyContent,
            gap,
            rowGap,
            columnGap,
          },
        ]}
      >
        {children}
      </Motion>
    </GlassView>
  );
}

const STYLE = StyleSheet.create({
  layout: { flexGrow: 1, alignSelf: "stretch" },
});
