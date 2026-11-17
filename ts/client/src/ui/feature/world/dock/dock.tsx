// @pxl/client/ui/feature/world/dock/dock.tsx

import { useRef, useState } from "react";
import { PixelRatio, StyleSheet, View, type ViewInstance } from "react-native";

import { GestureDetector } from "react-native-gesture-handler";

import type { f64 } from "@xpute/kit/abi/word.ts";
import { KEEPS_KEYBOARD } from "@/ui/shared/component/keyboard.tsx";
import { CHROME, COLOR } from "@/ui/shared/component/theme.ts";
import { DockMode, switchDockMode, useDockMode } from "./store.ts";
import { useDeckSwipe } from "@/ui/feature/world/deck.ts";
import { Card, DOCK_HEIGHT } from "./card.tsx";
import { SizePopover, usePopover } from "./popover.tsx";
import { Message, useMessage } from "./message.tsx";
import { STRIP, Strip } from "./strip.tsx";

export function Dock() {
  const mode = useDockMode();
  // A vertical swipe either way switches the mode. A swatch opens its sizes on
  // a hold, not a drag, so the swipe and the swatch never compete.
  const swipe = useDeckSwipe(switchDockMode, {});
  const message = useMessage(mode);
  const [barWidth, setBarWidth] = useState<f64>(0);
  const [stripWidth, setStripWidth] = useState<f64>(CHROME.control);
  const bar = useRef<ViewInstance | null>(null);
  const { popover, gesture } = usePopover(bar);

  const shownPopover = mode === DockMode.PALETTE ? popover : null;
  // Every card is as wide as the palette's strip. The cards stand on whole
  // pixels, because only there does a pill on the web cut its shadow cleanly.
  const width = Math.min(stripWidth, barWidth);
  const x = PixelRatio.roundToNearestPixel((barWidth - width) / 2);

  return (
    <GestureDetector gesture={swipe} touchAction="pan-x">
      <View
        ref={bar}
        {...KEEPS_KEYBOARD}
        onLayout={(e) => setBarWidth(e.nativeEvent.layout.width)}
        style={STYLE.bar}
      >
        {barWidth > 0 && (
          <>
            <Card
              key={DockMode.MESSAGE}
              pane={DockMode.MESSAGE}
              mode={mode}
              fill={COLOR.ground}
              x={x}
              width={width}
              spanPx={barWidth}
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
              spanPx={barWidth}
              onContentWidth={setStripWidth}
            >
              <Strip
                spanPx={barWidth}
                scrolls={!popover?.open}
                gesture={gesture}
              />
            </Card>

            {shownPopover && <SizePopover popover={shownPopover} />}
          </>
        )}
      </View>
    </GestureDetector>
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
