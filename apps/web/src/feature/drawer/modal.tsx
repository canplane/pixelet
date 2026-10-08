// @pxl/web/feature/drawer/modal.tsx

import { createContext, useContext, useMemo, useRef, useState } from "react";
import { useTranslation } from "react-i18next";
import { type LayoutChangeEvent, Pressable, StyleSheet, Text, View } from "react-native";

import type { f64, u8 } from "@xpute/kit/abi/word.ts";
import { decorKinds, decorSection, type PlaceableCube, placeableCubeId, placeableCubes, placeableCubeSection } from "@/catalog/cell/mod.ts";

import { useInHand, useUiStore } from "@/app/store/ui.ts";
import { useKernel } from "@/kernel/snapshot.ts";
import { ActiveTool } from "@/kernel/abi/cmd.spec.ts";
import { Panel } from "@platform/panel.tsx";
import { Glyph } from "@/shared/ui/glyph.tsx";
import { PanelBar, PanelBody, PanelScroll } from "@/shared/ui/panel.tsx";
import { Segmented } from "@platform/segmented.tsx";
import { chrome, color, hairline, textStyle } from "@/shared/ui/theme.ts";
import { SectionTitle } from "@/shared/ui/section-title.tsx";
import { CharacterPicker } from "@/feature/character/picker.tsx";
import { cubeLabel, CubeSwatch, decorGlyph, decorLabel, heldRing } from "@/catalog/cell/swatch.tsx";
import { LABEL } from "@platform/palette.ts";

/** Each section's items, in the catalog's order. */
function sectioned<S, T>(items: readonly T[], sectionOf: (item: T) => S): Map<S, T[]> {
  const sections = new Map<S, T[]>();
  for (const item of items) sections.set(sectionOf(item), [...(sections.get(sectionOf(item)) ?? []), item]);
  return sections;
}
const SECTIONS = sectioned(placeableCubes, placeableCubeSection);
const DECOR_SECTIONS = sectioned(decorKinds, decorSection);

// A press lands on 44 at least (decision/20260928-three-sizes-of-a-control.md); a circle cut a block's texture away, so a rounded square.
const GRID_GAP = 4;
const LIT_SCALE = 1.05;
const LIT_INK = 2;
const ITEM_RADIUS = 12;

/** Lit, an item reaches no further than halfway into the gap: a cell's `ITEM_SLOPE` plus `ITEM_BASE`. */
const ITEM_SLOPE = 1 / LIT_SCALE;
const ITEM_BASE = (GRID_GAP - 2 * (1 + LIT_INK)) / LIT_SCALE;
function itemOf(cell: f64): f64 {
  return ITEM_SLOPE * cell + ITEM_BASE;
}

/**
 * As many columns as fit a target each, the cells sized so the outer items'
 * edges meet the row's, as the other tabs' content does: the cells overhang by
 * the margin an item keeps inside its cell.
 */
function Grid({ head, children }: { head: React.ReactNode; children: (cell: f64) => React.ReactNode }) {
  const [width, setWidth] = useState(0);
  const columns = Math.max(1, Math.floor((width + GRID_GAP) / (chrome.target + GRID_GAP)));
  const cell = width > 0 ? (width - (columns - 1) * GRID_GAP - ITEM_BASE) / (columns - 1 + ITEM_SLOPE) : chrome.target;
  return (
    <View style={styles.section} onLayout={(e: LayoutChangeEvent) => setWidth(e.nativeEvent.layout.width)}>
      {head}
      <View style={[styles.grid, { marginHorizontal: -(cell - itemOf(cell)) / 2 }]}>{children(cell)}</View>
    </View>
  );
}

const TIP_POINT = 5;
/** Room for the longest name, centered on the item. */
const TIP_SPAN = 240;
/** Drawn solid and faded as one layer: faded apart, the point and the pill overlapped in a darker line where a device's pixels rounded them together. */
const TIP_FILL = color.black;
const TIP_OPACITY = 0.72;
const TIP_PAD_Y = 5;
/** A pill: ends rounded by half its height. */
const TIP_RADIUS = (textStyle.notice.lineHeight + 2 * TIP_PAD_Y) / 2;

/** Where an item's name is drawn: the sheet's top layer, over its bar, which a name inside the scroll could not reach. */
interface Naming {
  show: (label: string, item: View) => void;
  hide: () => void;
}
const NamingContext = createContext<Naming | null>(null);

interface Named {
  label: string;
  x: f64;
  y: f64;
}

/** Pictures only: names under every item read as a list. A press names it above the finger. */
function Item({ label, active, cell, onPress, children }: { label: string; active: boolean; cell: f64; onPress: () => void; children: React.ReactNode }) {
  const naming = useContext(NamingContext);
  const ref = useRef<View>(null);
  return (
    <Pressable
      ref={ref}
      onPress={onPress}
      onPressIn={() => ref.current && naming?.show(label, ref.current)}
      onPressOut={() => naming?.hide()}
      accessibilityRole="button"
      accessibilityLabel={label}
      aria-selected={active}
      style={[styles.cell, { width: cell, height: cell }]}
    >
      {children}
    </Pressable>
  );
}

function NameTip({ named, sheetHeight }: { named: Named; sheetHeight: f64 }) {
  return (
    <View pointerEvents="none" style={[styles.tipAnchor, { left: named.x - TIP_SPAN / 2, bottom: sheetHeight - named.y }]}>
      <View style={styles.tipShape}>
        <View needsOffscreenAlphaCompositing style={[StyleSheet.absoluteFill, styles.tipFill]}>
          <View style={styles.tipPill} />
          <View style={styles.tipPoint} />
        </View>
        <Text numberOfLines={1} style={styles.tip}>{named.label}</Text>
      </View>
    </View>
  );
}

function CubeItem({ cube, cell, onPick }: { cube: PlaceableCube; cell: f64; onPick: (cube: PlaceableCube) => void }) {
  const { t } = useTranslation();
  const active = useInHand({ tool: ActiveTool.Object, cube });

  return (
    <Item label={cubeLabel(t, cube)} active={active} cell={cell} onPress={() => onPick(cube)}>
      <CubeSwatch cube={cube} size={itemOf(cell)} radius={ITEM_RADIUS} style={active ? styles.markOn : styles.markOff} />
    </Item>
  );
}

function DecorItem({ kind, cell, onPick }: { kind: u8; cell: f64; onPick: (kind: u8) => void }) {
  const { t } = useTranslation();
  const active = useInHand({ tool: ActiveTool.Decor, kind });

  return (
    <Item label={decorLabel(t, kind)} active={active} cell={cell} onPress={() => onPick(kind)}>
      <View style={[styles.decor, { width: itemOf(cell), height: itemOf(cell) }, active ? [styles.markOn, styles.decorOn] : styles.markOff]}>
        <Glyph glyph={decorGlyph(kind)} size={chrome.glyph} color={LABEL.primary} />
      </View>
    </Item>
  );
}

/** A temporary way in: one marker, its words not kept yet. */
function MarkerItem({ cell, onPick }: { cell: f64; onPick: () => void }) {
  const { t } = useTranslation();
  const active = useKernel(({ holding: h }) => h.tool === ActiveTool.Marker);

  return (
    <Item label={t("blocks.marker")} active={active} cell={cell} onPress={onPick}>
      <View style={[styles.decor, { width: itemOf(cell), height: itemOf(cell) }, active ? [styles.markOn, styles.decorOn] : styles.markOff]}>
        <Glyph glyph="bubble.left" size={chrome.glyph} color={LABEL.primary} />
      </View>
    </Item>
  );
}

const enum DrawerTab {
  Cube = 0,
  Decor = 1,
  Marker = 2,
  Character = 3,
}

/** Cube, decor and marker picks close the sheet; character picks apply at once and leave it open. */
export function DrawerModal({ onClose }: { onClose: () => void }) {
  const { t } = useTranslation();
  const [tab, setTab] = useState<DrawerTab>(DrawerTab.Cube);
  const selectCube = useUiStore.getState().selectCube;
  const selectDecor = useUiStore.getState().selectDecor;
  const sheet = useRef<View>(null);
  const [sheetHeight, setSheetHeight] = useState(0);
  const [named, setNamed] = useState<Named | null>(null);
  // In the window, not against the sheet: a device's measureLayout leaves the scroll out.
  // A press counts, so a measure that answers after its release shows nothing.
  const press = useRef(0);
  const naming = useMemo<Naming>(() => ({
    show: (label, item) => {
      const at = ++press.current;
      sheet.current?.measureInWindow((sx, sy) =>
        item.measureInWindow((x, y, w) => {
          if (press.current === at) setNamed({ label, x: x - sx + w / 2, y: y - sy });
        })
      );
    },
    hide: () => {
      press.current++;
      setNamed(null);
    },
  }), []);

  function pickCube(cube: PlaceableCube): void {
    selectCube(cube);
    onClose();
  }

  function pickDecor(kind: u8): void {
    selectDecor(kind);
    onClose();
  }

  function pickMarker(): void {
    useUiStore.getState().selectMarker();
    onClose();
  }

  return (
    <Panel label={t("drawer.title")} onClose={onClose} half>
      <View ref={sheet} style={styles.sheet} onLayout={(e: LayoutChangeEvent) => setSheetHeight(e.nativeEvent.layout.height)}>
        <NamingContext.Provider value={naming}>
          <PanelBody
            bar={
              <PanelBar onClose={onClose}>
                <Segmented
                  value={tab}
                  onChange={setTab}
                  segments={[
                    { value: DrawerTab.Cube, label: t("drawer.tabs.cube") },
                    { value: DrawerTab.Decor, label: t("drawer.tabs.decor") },
                    { value: DrawerTab.Marker, label: t("drawer.tabs.marker") },
                    { value: DrawerTab.Character, label: t("drawer.tabs.character") },
                  ]}
                />
              </PanelBar>
            }
          >
            <PanelScroll key={tab}>
              {tab === DrawerTab.Cube &&
                [...SECTIONS].map(([section, cubes]) => (
                  <Grid key={section} head={<SectionTitle>{t(`drawer.sections.cube.${section}`)}</SectionTitle>}>
                    {(cell) => cubes.map((cube) => <CubeItem key={placeableCubeId(cube)} cube={cube} cell={cell} onPick={pickCube} />)}
                  </Grid>
                ))}

              {tab === DrawerTab.Decor &&
                [...DECOR_SECTIONS].map(([section, kinds]) => (
                  <Grid key={section} head={<SectionTitle>{t(`drawer.sections.decor.${section}`)}</SectionTitle>}>
                    {(cell) => kinds.map((kind) => <DecorItem key={kind} kind={kind} cell={cell} onPick={pickDecor} />)}
                  </Grid>
                ))}
              {tab === DrawerTab.Marker && (
                <Grid head={<SectionTitle>{t("drawer.sections.marker")}</SectionTitle>}>
                  {(cell) => <MarkerItem cell={cell} onPick={pickMarker} />}
                </Grid>
              )}

              {tab === DrawerTab.Character && <CharacterPicker />}
            </PanelScroll>
          </PanelBody>
        </NamingContext.Provider>
        {named && <NameTip named={named} sheetHeight={sheetHeight} />}
      </View>
    </Panel>
  );
}

const styles = StyleSheet.create({
  section: { gap: 8 },
  grid: { flexDirection: "row", flexWrap: "wrap", gap: GRID_GAP },
  cell: { alignItems: "center", justifyContent: "center" },
  sheet: { flex: 1 },
  tipAnchor: { position: "absolute", width: TIP_SPAN, alignItems: "center", paddingBottom: 2 },
  tipShape: { alignItems: "center", paddingBottom: TIP_POINT },
  tipFill: { opacity: TIP_OPACITY, alignItems: "center" },
  tipPill: { alignSelf: "stretch", flex: 1, backgroundColor: TIP_FILL, borderRadius: TIP_RADIUS },
  tip: {
    ...textStyle.notice,
    color: color.white,
    paddingHorizontal: 12,
    paddingVertical: TIP_PAD_Y,
  },
  tipPoint: {
    // Into the pill a pixel, so no rounding opens a gap; under one opacity the overlap does not show.
    marginTop: -1,
    width: 0,
    height: 0,
    borderLeftWidth: TIP_POINT,
    borderRightWidth: TIP_POINT,
    borderTopWidth: TIP_POINT,
    borderLeftColor: "transparent",
    borderRightColor: "transparent",
    borderTopColor: TIP_FILL,
  },
  markOff: { boxShadow: `0 0 0 1px ${hairline}` },
  markOn: { boxShadow: heldRing(color.ground, color.black, LIT_INK), transform: [{ scale: LIT_SCALE }] },
  decor: { borderRadius: ITEM_RADIUS, alignItems: "center", justifyContent: "center" },
  decorOn: { backgroundColor: color.faint },
});
