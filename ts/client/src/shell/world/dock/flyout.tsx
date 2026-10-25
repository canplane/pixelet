// @pxl/client/shell/world/dock/flyout.tsx

/** A swatch's sizes above the card: a press shows their seed, and held, it grows into them. */

import { useRef, useState } from "react";
import { useTranslation } from "react-i18next";
import { StyleSheet, View } from "react-native";

import type { f64, u32 } from "@xpute/kit/abi/word.ts";
import { round } from "@xpute/kit/math/scalar.ts";
import { Backing, BackingPlace } from "@/shell/ui/backing.tsx";
import { Motion } from "@/shell/ui/motion/motion.tsx";
import { SNAP } from "@/shell/ui/motion/transition.ts";
import { touches } from "@/shell/ui/touch.ts";
import { alpha, chrome, material, shadow } from "@/shell/ui/theme.ts";
import { holdTier } from "@/shell/store/dock.ts";
import { useKernel } from "@/shell/store/watch.ts";
import { OBJECT_TIERS } from "@/shell/world/object.spec.ts";
import { DOCK_HEIGHT } from "./card.tsx";
import { type SizeGesture, STRIP } from "./strip.tsx";

/** Above the card, where the sizes and their seed stand. */
const FLYOUT_LIFT_PX = 6;
const FLYOUT_GAP_PX = 8;
const FLYOUT_PAD_Y = 8;
const FLYOUT_WIDTH = chrome.target + 12;
const FLYOUT_PITCH = chrome.target + FLYOUT_GAP_PX;

/** A dot whose area halves per tier, so the finest stays visible: 28, 20, 14, 10. */
const MARK_COARSEST = 28;
function sizeMark(tier: u32): f64 {
  return round(MARK_COARSEST / Math.SQRT2 ** tier);
}

/** Finest first, so the coarsest stands nearest the swatch. */
const TIERS: readonly u32[] = Array.from({ length: OBJECT_TIERS }, (_, i) => OBJECT_TIERS - 1 - i);

/** A press on a swatch shows the sizes' seed; held, it grows into them. */
export const enum Sizes {
  Seed = 0,
  Open = 1,
}

/** The seed of the sizes, at their foot. */
const SEED = 10;

/** How long a key leaves the seed or the sizes up, long enough to read the one held. */
const CUE_MS = 900;

/** In the bar's frame, whose top is the card's; `bar` is where the bar is on the page. */
export interface Flyout {
  centerX: f64;
  bar: [f64, f64];
  hover: u32 | null;
  sizes: Sizes;
}

/** `x` and `y` on the page. */
function tierAt(at: Flyout, x: f64, y: f64): u32 | null {
  if (Math.abs(x - at.bar[0] - at.centerX) > FLYOUT_WIDTH / 2) return null;
  const top = at.bar[1] - FLYOUT_LIFT_PX - (2 * FLYOUT_PAD_Y + OBJECT_TIERS * FLYOUT_PITCH - FLYOUT_GAP_PX);
  const along = y - top - FLYOUT_PAD_Y;
  if (along < 0) return null;
  const row = Math.floor(along / FLYOUT_PITCH);
  if (row >= OBJECT_TIERS || along - row * FLYOUT_PITCH > chrome.target) return null;
  return TIERS[row];
}

/** Where the flyout stands, and what a swatch's gesture does to it; `bar` is the dock's, whose place it is measured from. */
export function useFlyout(bar: React.RefObject<View | null>): { flyout: Flyout | null; sizes: Omit<SizeGesture, "tap"> } {
  const [flyout, setFlyout] = useState<Flyout | null>(null);
  // A release can come before the swatch's place is measured.
  const pressing = useRef(false);
  const cueTimer = useRef<ReturnType<typeof setTimeout> | undefined>(undefined);

  const sizes: Omit<SizeGesture, "tap"> = {
    press: (swatch) => {
      pressing.current = true;
      swatch.measureInWindow((sx, _sy, sw) => {
        bar.current?.measureInWindow((bx, by) => {
          if (!pressing.current) return;
          setFlyout({ centerX: sx + sw / 2 - bx, bar: [bx, by], hover: null, sizes: Sizes.Seed });
        });
      });
    },
    cue: (swatch, open) => {
      clearTimeout(cueTimer.current);
      swatch.measureInWindow((sx, _sy, sw) => {
        bar.current?.measureInWindow((bx, by) => setFlyout({ centerX: sx + sw / 2 - bx, bar: [bx, by], hover: null, sizes: open ? Sizes.Open : Sizes.Seed }));
      });
      cueTimer.current = setTimeout(() => setFlyout(null), CUE_MS);
    },
    open: () => setFlyout((f) => f && { ...f, sizes: Sizes.Open }),
    move: (x, y) => {
      if (flyout?.sizes !== Sizes.Open) return;
      const hover = tierAt(flyout, x, y);
      if (hover !== flyout.hover) setFlyout({ ...flyout, hover });
    },
    lift: (x, y) => {
      const tier = flyout?.sizes === Sizes.Open ? tierAt(flyout, x, y) : null;
      if (tier !== null) holdTier(tier);
    },
    close: () => {
      pressing.current = false;
      setFlyout(null);
    },
  };

  return { flyout, sizes };
}

/** The seed or the sizes over the swatch. Drawn from the bar, not the swatch, whose scrolling strip would clip it. */
export function SizeFlyout({ flyout }: { flyout: Flyout }) {
  return (
    <View style={[styles.flyoutAt, flyout.sizes === Sizes.Open ? styles.flyoutTakes : touches.block, { left: flyout.centerX - FLYOUT_WIDTH / 2 }]}>
      {flyout.sizes === Sizes.Open ? <SizeMarks hover={flyout.hover} /> : <Backing fill={STRIP.ground} material={material.flyout} place={BackingPlace.OverMap} style={styles.seed} />}
    </View>
  );
}

/** The sizes open, a mark a tier. */
function SizeMarks({ hover }: { hover: u32 | null }) {
  const { t } = useTranslation();
  const objectTier = useKernel((s) => s.holding.tier);

  return (
    <Backing fill={STRIP.ground} material={material.flyout} place={BackingPlace.OverMap} style={styles.flyout}>
      {TIERS.map((tier) => {
        const mark = sizeMark(tier);
        return (
          <View key={tier} accessibilityLabel={t("world.dock.object_size", { n: OBJECT_TIERS - tier })} style={styles.flyoutItem}>
            <Motion to={{ scale: hover === tier ? 1.25 : 1 }} transition={SNAP}>
              <View
                style={[
                  { width: mark, height: mark, borderRadius: mark / 2 },
                  hover === tier ? styles.markHover : objectTier === tier ? styles.markHeld : styles.markIdle,
                ]}
              />
            </Motion>
          </View>
        );
      })}
    </Backing>
  );
}

const styles = StyleSheet.create({
  flyoutAt: {
    position: "absolute",
    bottom: DOCK_HEIGHT + FLYOUT_LIFT_PX,
    width: FLYOUT_WIDTH,
    zIndex: 10,
  },
  // Takes the pointer whole, so the map under it draws no hover.
  flyoutTakes: { pointerEvents: "box-only" },
  seed: { alignSelf: "center", width: SEED, height: SEED, borderRadius: SEED / 2, boxShadow: shadow.lift },
  flyout: {
    alignItems: "center",
    gap: FLYOUT_GAP_PX,
    paddingVertical: FLYOUT_PAD_Y,
    borderRadius: FLYOUT_WIDTH / 2,
    boxShadow: shadow.lift,
  },
  flyoutItem: { width: chrome.target, height: chrome.target, alignItems: "center", justifyContent: "center" },
  markIdle: { backgroundColor: alpha(STRIP.mark, 0.4) },
  markHeld: { backgroundColor: alpha(STRIP.mark, 0.8) },
  markHover: { backgroundColor: STRIP.mark, boxShadow: `0 0 0 1px ${STRIP.mark}` },
});
