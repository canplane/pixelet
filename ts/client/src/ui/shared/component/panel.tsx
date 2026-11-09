// @pxl/client/ui/shared/component/panel.tsx

import {
  createContext,
  type ReactNode,
  useContext,
  useRef,
  useState,
} from "react";
import { useTranslation } from "react-i18next";
import {
  PanResponder,
  Pressable,
  ScrollView,
  StyleSheet,
  View,
  type ViewProps,
} from "react-native";

import type { f64 } from "@xpute/kit/abi/word.ts";
import { Backing, BackingPlace } from "@/ui/shared/component/backing.tsx";
import { CircularButton } from "@/ui/shared/component/circular-button.tsx";
import { Motion } from "@/ui/shared/component/motion/motion.tsx";
import {
  ease,
  SLIDE,
  SLIDE_AWAY,
} from "@/ui/shared/component/motion/transition.ts";
import { LABEL } from "@/ui/shared/component/palette.ts";
import {
  alpha,
  chrome,
  color,
  layer,
  material,
} from "@/ui/shared/component/theme.ts";
import { touches } from "@/ui/shared/component/touch.ts";
import { useSafeArea } from "@/service/platform/safe-area.ts";
import { useWindowSize } from "@/service/platform/viewport.ts";

export const PANEL_CONTROL = chrome.target;
/** The inset of iOS 26 Maps' sheet. The top row's controls and the content
 * keep this distance from the sheet's sides. */
export const PANEL_EDGE = 16;
/** The height of the band above the top row, where the sheet's handle stands. */
export const PANEL_TOP = 20;

export const PANEL_SECTION_GAP = 24;
export const PANEL_HEAD_HEIGHT = PANEL_TOP + PANEL_CONTROL + PANEL_EDGE;
export const PANEL_CONTENT_TOP = PANEL_TOP + PANEL_CONTROL + PANEL_SECTION_GAP;

/** The sheet's corner radius, which makes its corners concentric with the
 * round controls in the top row. */
export const SHEET_RADIUS = PANEL_EDGE + PANEL_CONTROL / 2;

export interface PanelProps {
  label: string;
  onClose: () => void;
  half?: boolean;
  children: ReactNode;
}

/** The panel's host tells it whether it is open. The panel calls `onExited` once
 * it has slid away, and only then does the host stop drawing it. */
export const PanelPresence = createContext<{
  open: boolean;
  onExited: () => void;
}>({ open: true, onExited: () => {} });

/** The row shows no title: the panel's `label` names the sheet to assistive
 * technology. */
export function PanelBar({
  onClose,
  children,
  ...rest
}: ViewProps & {
  onClose: () => void;
  children: ReactNode;
}) {
  return (
    <View {...rest} style={styles.bar}>
      <View style={styles.barMain}>{children}</View>
      <PanelClose onClose={onClose} />
    </View>
  );
}

export function PanelBody({
  bar,
  children,
}: {
  bar: ReactNode;
  children: ReactNode;
}) {
  return (
    <View style={styles.body}>
      <View style={styles.scroll}>{children}</View>
      <Backing
        fill={color.ground}
        material={material.bar}
        place={BackingPlace.OverContent}
        style={styles.head}
      >
        {bar}
      </Backing>
    </View>
  );
}

function PanelClose({ onClose }: { onClose: () => void }) {
  const { t } = useTranslation();
  return (
    <CircularButton
      glyph="xmark"
      size={PANEL_CONTROL}
      place={BackingPlace.OnSheet}
      onPress={onClose}
      label={t("close")}
    />
  );
}

/** The scroller of one tab, under the tabs. Each tab must give its scroller
 * its own key: a scroller shared by the tabs keeps its offset when the tab
 * changes. */
export function PanelScroll({ children }: { children: ReactNode }) {
  const { bottom } = useSafeArea();
  return (
    <ScrollView
      style={styles.tabScroll}
      contentContainerStyle={[
        styles.tabContent,
        {
          paddingBottom: bottom + PANEL_EDGE,
        },
      ]}
    >
      {children}
    </ScrollView>
  );
}

const styles = StyleSheet.create({
  bar: {
    marginTop: PANEL_TOP,
    height: PANEL_CONTROL,
    paddingHorizontal: PANEL_EDGE,
    flexDirection: "row",
    alignItems: "center",
    gap: chrome.controlGap,
  },
  barMain: { flex: 1, height: PANEL_CONTROL, justifyContent: "center" },
  body: { flex: 1 },
  head: {
    position: "absolute",
    top: 0,
    left: 0,
    right: 0,
    height: PANEL_HEAD_HEIGHT,
  },
  tabScroll: { flex: 1 },
  tabContent: {
    paddingHorizontal: PANEL_EDGE,
    paddingTop: PANEL_CONTENT_TOP,
    gap: PANEL_SECTION_GAP,
  },
  scroll: { flex: 1, overflow: "hidden" },
});

/** The width of an iPad form sheet, which the sheet never exceeds. */
const MAX_WIDTH = 540;
const WHOLE_GAP = 12;
/** The speed, in points per millisecond, past which a released drag flings
 * the sheet on to the next stop. */
const FLING = 0.5;
const FOLLOW = ease(0, [0, 0, 1, 1]);
/** The size of an iPhone sheet's grabber. */
const HANDLE = { width: 36, height: 5 };
const TAP_SLOP = 8;

/**
 * Only the header drags the sheet. The sheet is always laid out at full height
 * and slid down to show half, so that a drag animates nothing but a transform.
 */
export function Panel({ label, onClose, half, children }: PanelProps) {
  const window = useWindowSize();
  const { top } = useSafeArea();
  const { open, onExited } = useContext(PanelPresence);
  const width = Math.min(window.width, MAX_WIDTH);
  const fullHeight = window.height - top - WHOLE_GAP;
  const halfHeight = window.height / 2;

  const [full, setFull] = useState(!half);
  // The body is as tall as the part of the sheet in view, so that its scroll
  // can reach its end. It grows as soon as the sheet does, and shrinks only
  // once the sheet has slid down to half.
  const [bodyFull, setBodyFull] = useState(full);
  const [drag, setDrag] = useState<f64 | null>(null);
  const shown = full ? fullHeight : halfHeight;

  const detent = (toFull: boolean) => {
    setFull(toFull);
    if (toFull) setBodyFull(true);
  };
  const stops = [
    { height: 0, settle: onClose },
    ...(half ? [{ height: halfHeight, settle: () => detent(false) }] : []),
    { height: fullHeight, settle: () => detent(true) },
  ];

  const now = {
    top: window.height - shown,
    center: window.width / 2,
    shown,
    full,
    half,
    stops,
  };
  const live = useRef(now);
  live.current = now;

  const responder = useRef(
    PanResponder.create({
      // The header claims a touch at the press. A drag upward soon leaves the
      // sheet, and its moves would then go to the world. The handle is not a
      // button, because a button would take its drags as well.
      onStartShouldSetPanResponder: (e) =>
        e.nativeEvent.pageY - live.current.top < PANEL_HEAD_HEIGHT,
      onPanResponderTerminationRequest: () => false,
      onPanResponderMove: (_, g) => setDrag(g.dy),
      onPanResponderRelease: (_, g) => {
        setDrag(null);
        const { top, center, shown, full, half, stops } = live.current;
        const tap = Math.abs(g.dx) < TAP_SLOP && Math.abs(g.dy) < TAP_SLOP;
        if (tap) {
          const onHandle =
            g.y0 - top < chrome.target &&
            Math.abs(g.x0 - center) < chrome.target / 2;
          if (half && onHandle) detent(!full);
          return;
        }
        const height = shown - g.dy;
        const stop =
          g.vy < -FLING
            ? stops.find((s) => s.height > shown)
            : g.vy > FLING
              ? stops.findLast((s) => s.height < shown)
              : stops.reduce((a, b) =>
                  Math.abs(b.height - height) < Math.abs(a.height - height)
                    ? b
                    : a,
                );
        stop?.settle();
      },
      onPanResponderTerminate: () => setDrag(null),
    }),
  ).current;

  const rest = fullHeight - shown;
  const offset = drag === null ? rest : Math.max(0, rest + drag);
  const away = { translateY: fullHeight + 16 };

  return (
    <View style={[sheetStyles.layer, open ? touches.pass : touches.block]}>
      <Motion
        from={{ opacity: 0 }}
        to={{ opacity: open ? 1 : 0 }}
        transition={open ? SLIDE : SLIDE_AWAY}
        style={StyleSheet.absoluteFill}
      >
        <Pressable
          accessible={false}
          onPress={onClose}
          style={sheetStyles.dim}
        />
      </Motion>
      <Motion
        from={away}
        to={open ? { translateY: offset } : away}
        transition={drag !== null ? FOLLOW : open ? SLIDE : SLIDE_AWAY}
        onSettled={open ? () => drag === null && setBodyFull(full) : onExited}
        style={[
          sheetStyles.sheet,
          {
            width,
            height: fullHeight,
            left: (window.width - width) / 2,
          },
        ]}
      >
        <View
          {...responder.panHandlers}
          accessibilityViewIsModal
          accessibilityLabel={label}
          style={[
            sheetStyles.body,
            {
              height: bodyFull ? fullHeight : halfHeight,
            },
          ]}
        >
          {children}
          <View style={sheetStyles.handle} />
        </View>
      </Motion>
    </View>
  );
}

const sheetStyles = StyleSheet.create({
  layer: { ...StyleSheet.absoluteFill, zIndex: layer.sheet },
  body: { width: "100%" },
  dim: { flex: 1, backgroundColor: alpha(color.black, 0.2) },
  sheet: {
    position: "absolute",
    bottom: 0,
    overflow: "hidden",
    borderTopLeftRadius: SHEET_RADIUS,
    borderTopRightRadius: SHEET_RADIUS,
    backgroundColor: color.ground,
    boxShadow: "0 -4px 20px rgba(0, 0, 0, 0.12)",
  },
  handle: {
    position: "absolute",
    top: (PANEL_TOP - HANDLE.height) / 2,
    left: "50%",
    marginLeft: -HANDLE.width / 2,
    ...HANDLE,
    borderRadius: HANDLE.height / 2,
    pointerEvents: "none",
    backgroundColor: LABEL.tertiary,
  },
});
