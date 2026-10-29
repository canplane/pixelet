// @pxl/client/shell/world/dock/dock.tsx

/**
 * The dock: the message's card and the palette's in one deck, a swipe
 * turning it, and over the palette the sizes a swatch's hold opens.
 */

import { useRef, useState } from "react";
import { PanResponder, PixelRatio, StyleSheet, View } from "react-native";

import type { f64 } from "@xpute/kit/abi/word.ts";
import { keepKeyboard } from "@/shell/ui/keyboard.tsx";
import { chrome, color } from "@/shell/ui/theme.ts";
import { DockMode, useDockStore } from "@/shell/store/dock.ts";
import { deckSwipe } from "@/shell/world/deck.ts";
import { Card, DOCK_HEIGHT } from "./card.tsx";
import { SizePopover, usePopover } from "./popover.tsx";
import { Message, useMessage } from "./message.tsx";
import { DRAG_PX, STRIP, Strip } from "./strip.tsx";

export function Dock() {
  const mode = useDockStore((s) => s.dockMode);
  // A swipe either way switches the mode; swatches open their sizes on a
  // hold, so the two never compete.
  const swipe = useRef(
    PanResponder.create({
      onMoveShouldSetPanResponderCapture: (_e, s) =>
        Math.abs(s.dy) > DRAG_PX && Math.abs(s.dy) > Math.abs(s.dx),
      ...deckSwipe(() => useDockStore.getState().switchDockMode(), {
        press: keepKeyboard,
      }),
    }),
  ).current;
  const message = useMessage(mode);
  const [barWidth, setBarWidth] = useState<f64>(0);
  const [stripWidth, setStripWidth] = useState<f64>(chrome.control);
  const bar = useRef<View | null>(null);
  const { popover, gesture } = usePopover(bar);

  const shownPopover = mode === DockMode.Palette ? popover : null;
  // Every card is as wide as the palette's tools; whole pixels, where a page's pill cuts its shadow cleanly.
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
            <Strip
              span={barWidth}
              scrolls={!popover?.open}
              gesture={gesture}
            />
          </Card>

          {shownPopover && <SizePopover popover={shownPopover} />}
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
