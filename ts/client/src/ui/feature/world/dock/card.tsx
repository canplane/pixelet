// @pxl/client/ui/feature/world/dock/card.tsx

import type { ReactNode } from "react";
import { StyleSheet } from "react-native";

import type { f64 } from "@xpute/kit/abi/word.ts";
import { BackingPlace } from "@/ui/shared/component/backing.tsx";
import { Motion } from "@/ui/shared/component/motion/motion.tsx";
import { SETTLE } from "@/ui/shared/component/motion/transition.ts";
import { TOUCH } from "@/ui/shared/component/touch.ts";
import { CHROME, SHADOW } from "@/ui/shared/component/theme.ts";
import type { DockMode } from "./store.ts";
import { Pill } from "@/ui/feature/world/pill.tsx";
import { card_pose, peek_below } from "@/ui/feature/world/deck.ts";

const PEEK_PX = 10;

export const DOCK_HEIGHT = CHROME.control + PEEK_PX;

/** Handlers that do nothing. The bar's swipe handles the drags on a card;
 * these only make the front card take its touches, so that none reaches the
 * world under it. */
const BAR_TOUCH = {};

const MODES = 2;

interface CardProps {
  pane: DockMode;
  mode: DockMode;
  fill: string;
  tint?: string;
  x: f64;
  width: f64;
  /** `spanPx` is the bar's width. */
  spanPx: f64;
  onContentWidth?: (w: f64) => void;
  children: ReactNode;
}

export function Card({
  pane,
  mode,
  fill,
  tint,
  x,
  width,
  spanPx,
  onContentWidth,
  children,
}: CardProps) {
  const depth = pane === mode ? 0 : 1;
  const pose = card_pose(depth, PEEK_PX);
  // The current mode's card always comes forward from behind, whatever chose
  // the mode: a swipe, a key or a marker's draft.
  return (
    <Motion
      to={pose.rest}
      transition={SETTLE}
      style={[
        STYLE.card,
        depth === 0 ? TOUCH.pass : TOUCH.block,
        {
          zIndex: pose.z_index,
        },
      ]}
    >
      <Pill
        x={x}
        width={width}
        height={CHROME.control}
        spanPx={spanPx}
        fill={fill}
        tint={tint}
        place={BackingPlace.OVER_MAP}
        shadow={SHADOW.lift}
        fade={pose.fade}
        fadeTransition={SETTLE}
        touch={depth === 0 ? BAR_TOUCH : undefined}
        touchBelow={depth === 0 ? peek_below(MODES, PEEK_PX) : 0}
        onContentWidth={onContentWidth}
      >
        {children}
      </Pill>
    </Motion>
  );
}

const STYLE = StyleSheet.create({
  card: {
    position: "absolute",
    left: 0,
    top: 0,
    right: 0,
    height: CHROME.control,
    transformOrigin: "center bottom",
  },
});
