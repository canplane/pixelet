// @pxl/web/feature/map/ui/overlay/mode-bar.tsx

import { useRef, useState } from "react";
import { Eraser, LayoutGrid, MessageCircle, MousePointer, Palette, Send, TreePine } from "lucide-react-native";
import { useTranslation } from "react-i18next";
import { Pressable, ScrollView, StyleSheet, type TextInput, useWindowDimensions, View } from "react-native";

import type { f64, u32, u8 } from "@xpute/core/abi/word.ts";
import { Glass } from "@platform/glass.tsx";
import { TextField } from "@/shared/ui/text-field.tsx";
import { alpha, color } from "@/shared/ui/theme.ts";
import { useAppContext } from "@/app/provider/context.ts";
import { useUiStore } from "@/app/store/ui-store.ts";
import { ActiveTool } from "@/kernel/abi/cmd.spec.ts";
import { decorName, type PlaceableCube, placeableCubeChannel, placeableCubeId, sameCube } from "@/catalog/cell/mod.ts";
import type { DecorId } from "@/catalog/cell/decor.spec.ts";
import { useKernel } from "@/kernel/shell.ts";
import { CubeSwatch } from "./cube-swatch.tsx";

type ModeId = "message" | "palette";

const SEND_FADE_MS = 200;

// The height of both pills — the map view (view.tsx) stands the toasts
// clear of it.
export const MODE_BAR_HEIGHT_PX = 48;
/** A tool or swatch's pitch in the strip (AGENTS.md "Sizing UI against
 * something": the Instagram DM icon row). */
const PITCH = 38;
const GLYPH = 20;
/** How far above the bar the size flyout's bottom stands. */
const FLYOUT_LIFT_PX = 20;

/** A size tier's mark in the flyout, finest the smallest. */
function sizeMark(tier: u32): number {
  return tier === 0 ? 24 : tier === 1 ? 16 : tier === 2 ? 8 : 4;
}

/** The tiers top to bottom: coarsest first, as the marks shrink downward. */
function sizeTiers(count: u32): u32[] {
  return Array.from({ length: count }, (_, i) => count - 1 - i);
}

/** Where the size flyout stands: over a swatch's center, from the bar's left. */
type FlyoutAt = f64;

/** The size flyout over the swatch it was opened from: a tier tapped is the
 * size placed from then on. Drawn from the bar rather than from the swatch,
 * which sits in a horizontally scrolling strip that clips what stands
 * outside it. */
function SizeFlyout({ left, onClose }: { left: FlyoutAt; onClose: () => void }) {
  const object_tier = useUiStore((s) => s.object_tier);
  const setObjectTier = useUiStore.getState().setObjectTier;
  const sizes = useKernel((s) => s.object_sizes);

  return (
    <Glass fill={alpha(color.neutral900, 0.8)} tint={DARK_TINT} style={[styles.flyout, { left }]}>
      {sizeTiers(sizes.length).map((tier) => {
        const mark = sizeMark(tier);
        return (
          <Pressable
            key={tier}
            onPress={() => {
              setObjectTier(tier);
              onClose();
            }}
            accessibilityRole="button"
            accessibilityLabel={`${sizes[tier]}u`}
            style={styles.flyoutItem}
          >
            <View style={[styles.mark, { width: mark, height: mark }, object_tier === tier ? styles.markHeld : styles.markIdle]} />
          </Pressable>
        );
      })}
    </Glass>
  );
}

/** A cube in the strip: a tap takes it in hand, a long press takes it and
 * opens the size flyout over it. */
function ColorSwatchButton({ cube, onSizes }: { cube: PlaceableCube; onSizes: (swatch: View) => void }) {
  const { t } = useTranslation();
  const activeTool = useUiStore((s) => s.activeTool);
  const held = useUiStore((s) => s.cube);
  const selectCube = useUiStore.getState().selectCube;
  const ref = useRef<View | null>(null);

  const selected = activeTool === ActiveTool.Object && sameCube(held, cube);

  return (
    <Pressable
      ref={ref}
      onPress={() => selectCube(cube)}
      onLongPress={() => {
        selectCube(cube);
        if (ref.current) onSizes(ref.current);
      }}
      delayLongPress={250}
      accessibilityRole="button"
      accessibilityLabel={t(`cubes.${placeableCubeId(cube)}`)}
      style={styles.slot}
    >
      <CubeSwatch cube={cube} size={GLYPH} radius={GLYPH / 2} style={selected ? styles.swatchSelected : styles.swatchIdle} />
    </Pressable>
  );
}

// The hotbar's decor slot(s) — reinstated (2026-09-07) now that recentPicks
// (ui-store.ts) is one shared MRU list across cube and decor: unlike a
// ColorSwatchButton, a decor pick has no size flyout of its own (DECOR_SIZE
// is fixed, pxl-kernel layer/object/size.rs), so this stays a plain tap button.
function DecorIconButton({ kind }: { kind: u8 }) {
  const { t } = useTranslation();
  const activeTool = useUiStore((s) => s.activeTool);
  const held = useUiStore((s) => s.decor);
  const selectDecor = useUiStore.getState().selectDecor;
  const name = decorName(kind);
  const Icon = DECOR_ICONS[name];
  const active = activeTool === ActiveTool.Decor && held === kind;

  return (
    <ToolButton label={t(`decors.${name}`)} active={active} onPress={() => selectDecor(kind)}>
      <Icon size={GLYPH} strokeWidth={2} color={active ? color.white : alpha(color.white, 0.6)} />
    </ToolButton>
  );
}

// Icon per decor kind, by name — TypeScript enforces one entry per DecorId, so
// adding a new decor kind (decor.spec.ts's own decor_types) can't silently ship
// without a button icon for it. Lives here (this file's own DecorIconButton
// is one consumer) and is also imported by drawer-modal.tsx's own
// DecorGridButton, so a new decor kind's icon shows up in both the hotbar
// and the drawer for free.
export const DECOR_ICONS: Record<DecorId, typeof TreePine> = {
  tree: TreePine,
};

/** A round slot in the dark strip, lit while its tool is the one in hand. */
function ToolButton({ label, active, onPress, children }: { label: string; active?: boolean; onPress: () => void; children: React.ReactNode }) {
  return (
    <Pressable onPress={onPress} accessibilityRole="button" accessibilityLabel={label} style={[styles.slot, styles.round, active && styles.toolActive]}>
      {children}
    </Pressable>
  );
}

export function ModeBar() {
  const { t } = useTranslation();
  const { ui } = useAppContext();
  const { width } = useWindowDimensions();
  const [mode, setMode] = useState<ModeId>("palette");
  const activeTool = useUiStore((s) => s.activeTool);
  const set_activeTool = useUiStore.getState().set_activeTool;
  const recentPicks = useUiStore((s) => s.recentPicks);
  const append_chat_line = useUiStore.getState().append_chat_line;
  const [messageText, setMessageText] = useState("");
  const [isSending, setIsSending] = useState(false);
  const [flyoutAt, setFlyoutAt] = useState<FlyoutAt | null>(null);
  const inputRef = useRef<TextInput | null>(null);
  const bar = useRef<View | null>(null);

  function openSizes(swatch: View) {
    swatch.measureInWindow((sx, _sy, sw) => {
      bar.current?.measureInWindow((bx) => setFlyoutAt(sx + sw / 2 - bx));
    });
  }

  function sendMessage() {
    const text = messageText.trim();
    if (text === "" || isSending) return;
    setIsSending(true);
    setTimeout(() => {
      append_chat_line(text);
      setMessageText("");
      setIsSending(false);
      inputRef.current?.focus();
    }, SEND_FADE_MS);
  }

  const messageExpanded = mode === "message";
  const paletteExpanded = mode === "palette";
  // Reuses the palette pill's own idle collapsed-icon slot as the send
  // button rather than adding a separate one inside the message pill — the
  // same idle-slot-reuse pattern as the size flyout anchoring per-swatch
  // instead of to one shared control (see AGENTS.md).
  const showSendInPaletteSlot = messageExpanded && !paletteExpanded && messageText.trim() !== "";

  return (
    <View ref={bar} style={styles.bar}>
      {flyoutAt !== null && paletteExpanded && <SizeFlyout left={flyoutAt} onClose={() => setFlyoutAt(null)} />}

      <Glass fill={color.white} interactive={!messageExpanded} style={[styles.messagePill, messageExpanded && styles.messagePillOpen]}>
        <Pressable
          onPress={() => setMode("message")}
          accessibilityRole="button"
          accessibilityLabel={t("map.mode_bar.message_tooltip")}
          style={messageExpanded ? styles.messageIconOpen : styles.messageIcon}
        >
          <MessageCircle size={GLYPH} strokeWidth={2} color={alpha(color.black, 0.5)} />
        </Pressable>

        {messageExpanded && (
          <View style={styles.messageField}>
            <TextField
              ref={inputRef}
              value={messageText}
              onChangeText={setMessageText}
              onSubmit={sendMessage}
              placeholder={t("map.mode_bar.message_placeholder")}
              placeholderTextColor={alpha(color.black, 0.4)}
              autoFocus
              style={[styles.messageInput, isSending && styles.sending]}
            />
          </View>
        )}
      </Glass>

      <View style={[styles.palettePill, { maxWidth: width - 92 }]}>
        {!paletteExpanded && (
          showSendInPaletteSlot
            ? (
              <Pressable
                onPress={sendMessage}
                disabled={isSending}
                accessibilityRole="button"
                accessibilityLabel={t("map.mode_bar.send")}
                style={({ pressed }) => [styles.slotButton, styles.send, pressed && styles.sendPressed]}
              >
                <Send size={GLYPH} strokeWidth={2} color={color.white} />
              </Pressable>
            )
            : (
              <Pressable
                onPress={() => setMode("palette")}
                accessibilityRole="button"
                accessibilityLabel={t("map.mode_bar.palette_tooltip")}
                style={styles.slotButton}
              >
                <Glass fill={color.neutral900} tint={DARK_TINT} interactive style={styles.slotGround}>
                  <Palette size={GLYPH} strokeWidth={2} color={alpha(color.white, 0.6)} />
                </Glass>
              </Pressable>
            )
        )}

        {paletteExpanded && (
          <Glass fill={color.neutral900} tint={DARK_TINT} style={styles.strip}>
            <View style={styles.group}>
              <ToolButton label={t("map.mode_bar.pointer")} active={activeTool === ActiveTool.Pointer} onPress={() => set_activeTool(ActiveTool.Pointer)}>
                <MousePointer size={GLYPH} strokeWidth={2} color={activeTool === ActiveTool.Pointer ? color.white : alpha(color.white, 0.6)} />
              </ToolButton>
              <ToolButton label={t("map.mode_bar.eraser")} active={activeTool === ActiveTool.Eraser} onPress={() => set_activeTool(ActiveTool.Eraser)}>
                <Eraser size={GLYPH} strokeWidth={2} color={activeTool === ActiveTool.Eraser ? color.white : alpha(color.white, 0.6)} />
              </ToolButton>
            </View>

            <View style={styles.divider} />

            <ScrollView horizontal showsHorizontalScrollIndicator={false} onScrollBeginDrag={() => setFlyoutAt(null)} style={styles.picks} contentContainerStyle={styles.picksContent}>
              {recentPicks.map((pick) =>
                pick.tool === ActiveTool.Object
                  ? <ColorSwatchButton key={`${placeableCubeChannel(pick.cube)}:${pick.cube.type}`} cube={pick.cube} onSizes={openSizes} />
                  : <DecorIconButton key={`decor:${pick.kind}`} kind={pick.kind} />
              )}
            </ScrollView>

            <View style={styles.divider} />

            {
              // Opens the full catalog: every terrain-styled block
              // type and decor kind that hasn't (yet) earned a hotbar
              // slot, plus Character — Minecraft's own hotbar/
              // inventory relationship. Outside the scrolling strip
              // above so it's always reachable regardless of scroll
              // position.
            }
            <View style={styles.group}>
              <ToolButton label={t("map.mode_bar.catalog_tooltip")} onPress={ui.openDrawerModal}>
                <LayoutGrid size={GLYPH} strokeWidth={2} color={alpha(color.white, 0.6)} />
              </ToolButton>
            </View>
          </Glass>
        )}
      </View>
    </View>
  );
}

const PILL = { height: MODE_BAR_HEIGHT_PX, borderRadius: MODE_BAR_HEIGHT_PX / 2 };

/** The strip's glass, where there is glass: dark enough that its white
 * glyphs read over a bright map. */
const DARK_TINT = alpha(color.neutral900, 0.6);

const styles = StyleSheet.create({
  bar: { flexDirection: "row", alignItems: "center", justifyContent: "center", gap: 10, width: "100%", minWidth: 0 },

  messagePill: {
    ...PILL,
    flexDirection: "row",
    alignItems: "center",
    minWidth: 0,
    flexShrink: 1,
    overflow: "hidden",
    boxShadow: "0 6px 20px rgba(0, 0, 0, 0.25)",
  },
  messagePillOpen: { paddingLeft: 8 },
  messageIcon: { width: MODE_BAR_HEIGHT_PX, height: MODE_BAR_HEIGHT_PX, alignItems: "center", justifyContent: "center" },
  messageIconOpen: { width: 36, height: 36, borderRadius: 18, alignItems: "center", justifyContent: "center" },
  messageField: { flexDirection: "row", alignItems: "center", minWidth: 0, flexShrink: 1, paddingHorizontal: 8 },
  messageInput: { width: 384, maxWidth: "100%", minWidth: 0, color: color.black },
  sending: { opacity: 0 },

  // Its children paint the pill — the collapsed slot or the strip — and are
  // pill-shaped themselves, so the shadow follows this box's own radius.
  palettePill: { ...PILL, flexDirection: "row", alignItems: "center", flexShrink: 0, boxShadow: "0 3px 6px rgba(0, 0, 0, 0.25)" },
  slotButton: { ...PILL, width: MODE_BAR_HEIGHT_PX, alignItems: "center", justifyContent: "center" },
  slotGround: { ...PILL, alignSelf: "stretch", alignItems: "center", justifyContent: "center" },
  send: { backgroundColor: color.orange500, boxShadow: `0 4px 6px ${alpha(color.orange500, 0.4)}` },
  sendPressed: { transform: [{ scale: 0.95 }] },

  strip: { ...PILL, flexDirection: "row", alignItems: "center", minWidth: 0, flexShrink: 1 },
  group: { flexDirection: "row", alignItems: "center", padding: 4, flexShrink: 0 },
  divider: { width: 1, height: 28, backgroundColor: alpha(color.white, 0.15), flexShrink: 0 },
  picks: { flexShrink: 1, minWidth: 0 },
  picksContent: { alignItems: "center", paddingHorizontal: 4 },

  slot: { width: PITCH, height: PITCH, alignItems: "center", justifyContent: "center", flexShrink: 0 },
  round: { borderRadius: PITCH / 2 },
  toolActive: { backgroundColor: alpha(color.white, 0.2) },
  swatchIdle: { boxShadow: `0 0 0 1px ${alpha(color.white, 0.15)}` },
  swatchSelected: { boxShadow: `0 0 0 1px ${color.neutral900}, 0 0 0 2px ${color.white}`, transform: [{ scale: 1.1 }] },

  flyout: {
    position: "absolute",
    bottom: MODE_BAR_HEIGHT_PX + FLYOUT_LIFT_PX,
    transform: [{ translateX: "-50%" }],
    zIndex: 10,
    alignItems: "center",
    gap: 6,
    paddingVertical: 8,
    paddingHorizontal: 6,
    borderRadius: 999,
    boxShadow: "0 6px 20px rgba(0, 0, 0, 0.25)",
  },
  flyoutItem: { width: 40, height: 40, alignItems: "center", justifyContent: "center" },
  mark: { borderRadius: 2 },
  markIdle: { backgroundColor: alpha(color.white, 0.4) },
  markHeld: { backgroundColor: color.white },
});
