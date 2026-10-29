// @pxl/client/shell/world/dock/card.tsx

/** A card of the dock's deck, one a mode: the chosen one in front, the other peeking out under it. */

import type { ReactNode } from "react";
import { StyleSheet } from "react-native";

import type { f64 } from "@xpute/kit/abi/word.ts";
import { BackingPlace } from "@/shell/ui/backing.tsx";
import { Motion } from "@/shell/ui/motion/motion.tsx";
import { SETTLE } from "@/shell/ui/motion/transition.ts";
import { touches } from "@/shell/ui/touch.ts";
import { chrome, shadow } from "@/shell/ui/theme.ts";
import type { DockMode } from "./store.ts";
import { Pill } from "@/shell/world/pill.tsx";
import { cardPose, peekBelow } from "@/shell/world/deck.ts";

const PEEK_PX = 10;

/** The dock stands tall enough for the card behind to peek out under the front one. */
export const DOCK_HEIGHT = chrome.control + PEEK_PX;

/** The bar's swipe takes the card's drags; this only keeps a touch on it off the map. */
const BAR_TOUCH = {};

/** The dock's cards, one a mode. */
const MODES = 2;

interface CardProps {
  pane: DockMode;
  mode: DockMode;
  fill: string;
  tint?: string;
  x: f64;
  width: f64;
  /** The bar's width. */
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
  // However it was chosen, the chosen card comes forward from behind.
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
