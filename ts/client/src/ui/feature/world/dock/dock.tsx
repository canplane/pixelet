// @pxl/client/ui/feature/world/dock/dock.tsx

import { useRef, useState } from "react";
import { PanResponder, PixelRatio, StyleSheet, View } from "react-native";

import type { f64 } from "@xpute/kit/abi/word.ts";
import { keep_keyboard } from "@/ui/shared/component/keyboard.tsx";
import { chrome, color } from "@/ui/shared/component/theme.ts";
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
  const [barWidth, setBarWidth] = useState<f64>(0);
  const [stripWidth, setStripWidth] = useState<f64>(chrome.control);
  const bar = useRef<View | null>(null);
  const { popover, gesture } = usePopover(bar);

  const shown_popover = mode === DockMode.Palette ? popover : null;
  // Every card is as wide as the palette's strip. The cards stand on whole
  // pixels, because only there does a pill on a page cut its shadow cleanly.
  const width = Math.min(stripWidth, barWidth);
  const x = PixelRatio.roundToNearestPixel((barWidth - width) / 2);

  return (
    <View
      ref={bar}
      {...swipe.panHandlers}
      onLayout={(e) => setBarWidth(e.nativeEvent.layout.width)}
      style={styles.bar}
    >
      {barWidth > 0 && (
        <>
          <Card
            key={DockMode.Message}
            pane={DockMode.Message}
            mode={mode}
            fill={color.ground}
            x={x}
            width={width}
            span={barWidth}
          >
            <Message message={message} width={width} />
          </Card>

          <Card
            key={DockMode.Palette}
            pane={DockMode.Palette}
            mode={mode}
            fill={STRIP.ground}
            tint={STRIP.ground}
            x={x}
            width={width}
            span={barWidth}
            onContentWidth={setStripWidth}
          >
            <Strip span={barWidth} scrolls={!popover?.open} gesture={gesture} />
          </Card>

          {shown_popover && <SizePopover popover={shown_popover} />}
        </>
      )}
    </View>
  );
}

const styles = StyleSheet.create({
  bar: {
    height: DOCK_HEIGHT,
    width: "100%",
    minWidth: 0,
    pointerEvents: "box-none",
  },
});
