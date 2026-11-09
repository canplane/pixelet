// @pxl/client/ui/feature/world/dock/card.tsx

import type { ReactNode } from "react";
import { StyleSheet } from "react-native";

import type { f64 } from "@xpute/kit/abi/word.ts";
import { BackingPlace } from "@/ui/shared/component/backing.tsx";
import { Motion } from "@/ui/shared/component/motion/motion.tsx";
import { SETTLE } from "@/ui/shared/component/motion/transition.ts";
import { touches } from "@/ui/shared/component/touch.ts";
import { chrome, shadow } from "@/ui/shared/component/theme.ts";
import type { DockMode } from "./store.ts";
import { Pill } from "@/ui/feature/world/pill.tsx";
import { cardPose, peekBelow } from "@/ui/feature/world/deck.ts";

const PEEK_PX = 10;

export const DOCK_HEIGHT = chrome.control + PEEK_PX;

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
  /** `span` is the bar's width. */
  span: f64;
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
  span,
  onContentWidth,
  children,
}: CardProps) {
  const depth = pane === mode ? 0 : 1;
  const pose = cardPose(depth, PEEK_PX);
  // The current mode's card always comes forward from behind, whatever chose
  // the mode: a swipe, a key or a marker's draft.
  return (
    <Motion
      to={pose.rest}
      transition={SETTLE}
      style={[
        styles.card,
        depth === 0 ? touches.pass : touches.block,
        {
          zIndex: pose.zIndex,
        },
      ]}
    >
      <Pill
        x={x}
        width={width}
        height={chrome.control}
        span={span}
        fill={fill}
        tint={tint}
        place={BackingPlace.OverMap}
        shadow={shadow.lift}
        fade={pose.fade}
        fadeTransition={SETTLE}
        touch={depth === 0 ? BAR_TOUCH : undefined}
        touchBelow={depth === 0 ? peekBelow(MODES, PEEK_PX) : 0}
        onContentWidth={onContentWidth}
      >
        {children}
      </Pill>
    </Motion>
  );
}

const styles = StyleSheet.create({
  card: {
    position: "absolute",
    left: 0,
    top: 0,
    right: 0,
    height: chrome.control,
    transformOrigin: "center bottom",
  },
});
