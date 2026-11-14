// @pxl/client/ui/feature/world/dock/dock.tsx

import { useRef, useState } from "react";
import { PanResponder, PixelRatio, StyleSheet, View } from "react-native";

import type { f64 } from "@xpute/kit/abi/word.ts";
import { keep_keyboard } from "@/ui/shared/component/keyboard.tsx";
import { CHROME, COLOR } from "@/ui/shared/component/theme.ts";
import { DockMode, switch_dock_mode, useDockMode } from "./store.ts";
import { deck_swipe } from "@/ui/feature/world/deck.ts";
import { Card, DOCK_HEIGHT } from "./card.tsx";
import { SizePopover, usePopover } from "./popover.tsx";
import { Message, useMessage } from "./message.tsx";
import { DRAG_PX, STRIP, Strip } from "./strip.tsx";

export function Dock() {
  const mode = useDockMode();
  // A vertical swipe either way switches the mode. A swatch opens its sizes on
  // a hold, not a drag, so the swipe and the swatch never compete.
  const swipe = useRef(
    PanResponder.create({
      onMoveShouldSetPanResponderCapture: (_e, s) =>
        Math.abs(s.dy) > DRAG_PX && Math.abs(s.dy) > Math.abs(s.dx),
      ...deck_swipe(switch_dock_mode, {
        press: keep_keyboard,
      }),
    }),
  ).current;
  const message = useMessage(mode);
  const [bar_width, set_bar_width] = useState<f64>(0);
  const [strip_width, set_strip_width] = useState<f64>(CHROME.control);
  const bar = useRef<View | null>(null);
  const { popover, gesture } = usePopover(bar);

  const shown_popover = mode === DockMode.PALETTE ? popover : null;
  // Every card is as wide as the palette's strip. The cards stand on whole
  // pixels, because only there does a pill on a page cut its shadow cleanly.
  const width = Math.min(strip_width, bar_width);
  const x = PixelRatio.roundToNearestPixel((bar_width - width) / 2);

  return (
    <View
      ref={bar}
      {...swipe.panHandlers}
      onLayout={(e) => set_bar_width(e.nativeEvent.layout.width)}
      style={STYLE.bar}
    >
      {bar_width > 0 && (
        <>
          <Card
            key={DockMode.MESSAGE}
            pane={DockMode.MESSAGE}
            mode={mode}
            fill={COLOR.ground}
            x={x}
            width={width}
            spanPx={bar_width}
          >
            <Message message={message} width={width} />
          </Card>

          <Card
            key={DockMode.PALETTE}
            pane={DockMode.PALETTE}
            mode={mode}
            fill={STRIP.ground}
            tint={STRIP.ground}
            x={x}
            width={width}
            spanPx={bar_width}
            onContentWidth={set_strip_width}
          >
            <Strip spanPx={bar_width} scrolls={!popover?.open} gesture={gesture} />
          </Card>

          {shown_popover && <SizePopover popover={shown_popover} />}
        </>
      )}
    </View>
  );
}

const STYLE = StyleSheet.create({
  bar: {
    height: DOCK_HEIGHT,
    width: "100%",
    minWidth: 0,
    pointerEvents: "box-none",
  },
});
