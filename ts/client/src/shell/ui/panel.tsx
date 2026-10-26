// @pxl/client/shell/ui/panel.tsx

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
import { Backing, BackingPlace } from "@/shell/ui/backing.tsx";
import { CircularButton } from "@/shell/ui/circular-button.tsx";
import { Motion } from "@/shell/ui/motion/motion.tsx";
import { ease, SLIDE, SLIDE_AWAY } from "@/shell/ui/motion/transition.ts";
import { LABEL } from "@/shell/ui/palette.ts";
import { alpha, chrome, color, layer, material } from "@/shell/ui/theme.ts";
import { touches } from "@/shell/ui/touch.ts";
import { bracket } from "@/bracket.tsx";

export const PANEL_CONTROL = chrome.target;
/** iOS 26 Maps' sheet inset, for the top row's controls and the content's sides. */
export const PANEL_EDGE = 16;
/** Above the top row: the band the sheet's handle stands in. */
export const PANEL_TOP = 20;

export const PANEL_SECTION_GAP = 24;
export const PANEL_HEAD_HEIGHT = PANEL_TOP + PANEL_CONTROL + PANEL_EDGE;
export const PANEL_CONTENT_TOP = PANEL_TOP + PANEL_CONTROL + PANEL_SECTION_GAP;

/** Concentric with the round controls in the top row. */
export const SHEET_RADIUS = PANEL_EDGE + PANEL_CONTROL / 2;

export interface PanelProps {
  label: string;
  onClose: () => void;
  half?: boolean;
  children: ReactNode;
}

/** A host drops a panel only after `gone`, once it has slid away. */
export const PanelPresence = createContext<{ open: boolean; gone: () => void }>(
  { open: true, gone: () => {} },
);

/** The top row: the sheet's own control where a title would stand — its tabs or
 * its search field — and close. The panel's `label` names the sheet. */
export function PanelBar(
  { onClose, children, ...rest }: ViewProps & {
    onClose: () => void;
    children: ReactNode;
  },
) {
  return (
    <View {...rest} style={styles.bar}>
      <View style={styles.barMain}>{children}</View>
      <PanelClose onClose={onClose} />
    </View>
  );
}

export function PanelBody(
  { bar, children }: { bar: ReactNode; children: ReactNode },
) {
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

/** A tab's scroller under the tabs; keyed per tab, since a shared one kept its offset across a switch. */
export function PanelScroll({ children }: { children: ReactNode }) {
  const { bottom } = bracket.view.useSafeArea();
  return (
    <ScrollView
      style={styles.tabScroll}
      contentContainerStyle={[styles.tabContent, {
        paddingBottom: bottom + PANEL_EDGE,
      }]}
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

/**
 * A panel on a page and on Android: a bottom sheet over a dimmed map, dragged by its header only. Always laid
 * out full height and slid down to show half, so drags animate transform
 * only. Max width 540, an iPad form sheet's.
 */
const MAX_WIDTH = 540;
const WHOLE_GAP = 12;
/** Points per millisecond. */
const FLING = 0.5;
const FOLLOW = ease(0, [0, 0, 1, 1]);
/** An iPhone sheet's grabber. */
const HANDLE = { width: 36, height: 5 };
const TAP_SLOP = 8;

export function Panel({ label, onClose, half, children }: PanelProps) {
  const window = bracket.view.useWindowSize();
  const { top } = bracket.view.useSafeArea();
  const { open, gone } = useContext(PanelPresence);
  const width = Math.min(window.width, MAX_WIDTH);
  const fullHeight = window.height - top - WHOLE_GAP;
  const halfHeight = window.height / 2;

  const [full, setFull] = useState(!half);
  // Full height as soon as it grows, back at half once it has slid down, so the scroll reaches its end.
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
      // Claimed at the press: a drag up leaves the sheet, and its moves would go
      // to the map. The handle is no button, which would take its drags too.
      onStartShouldSetPanResponder: (e) =>
        e.nativeEvent.pageY - live.current.top < PANEL_HEAD_HEIGHT,
      onPanResponderTerminationRequest: () => false,
      onPanResponderMove: (_, g) => setDrag(g.dy),
      onPanResponderRelease: (_, g) => {
        setDrag(null);
        const { top, center, shown, full, half, stops } = live.current;
        const tap = Math.abs(g.dx) < TAP_SLOP && Math.abs(g.dy) < TAP_SLOP;
        if (tap) {
          const onHandle = g.y0 - top < chrome.target &&
            Math.abs(g.x0 - center) < chrome.target / 2;
          if (half && onHandle) detent(!full);
          return;
        }
        const height = shown - g.dy;
        const stop = g.vy < -FLING
          ? stops.find((s) => s.height > shown)
          : g.vy > FLING
          ? stops.findLast((s) => s.height < shown)
          : stops.reduce((
            a,
            b,
          ) => (Math.abs(b.height - height) < Math.abs(a.height - height)
            ? b
            : a)
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
        onSettled={open ? () => drag === null && setBodyFull(full) : gone}
        style={[sheetStyles.sheet, {
          width,
          height: fullHeight,
          left: (window.width - width) / 2,
        }]}
      >
        <View
          {...responder.panHandlers}
          accessibilityViewIsModal
          accessibilityLabel={label}
          style={[sheetStyles.body, {
            height: bodyFull ? fullHeight : halfHeight,
          }]}
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
