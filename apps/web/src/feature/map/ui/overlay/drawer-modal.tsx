// @pxl/web/feature/map/ui/overlay/drawer-modal.tsx

import { useState } from "react";
import { useTranslation } from "react-i18next";
import { Pressable, ScrollView, StyleSheet, Text, View } from "react-native";

import type { u8 } from "@xpute/core/abi/word.ts";
import { type ChannelName, decorKinds, decorName, type PlaceableCube, placeableCubeChannel, placeableCubeId, placeableCubes } from "@/catalog/cell/mod.ts";

import type { BaseModalProps } from "@/app/type/ui.type.ts";
import { useInHand, useUiStore } from "@/app/store/ui-store.ts";
import { ActiveTool } from "@/kernel/abi/cmd.spec.ts";
import { useSafeArea } from "@platform/safe-area.ts";
import { Panel } from "@platform/panel.tsx";
import { Glyph } from "@/shared/ui/glyph.tsx";
import { PANEL_EDGE, PANEL_SECTION_GAP, PANEL_TABS_ROOM, PanelBody, PanelHeader } from "@/shared/ui/panel.tsx";
import { Segmented } from "@platform/segmented.tsx";
import { chrome, color, hairline, sectionTitle, textStyle } from "@/shared/ui/theme.ts";
import { CharacterPicker } from "@/feature/character/picker.tsx";
import { cubeLabel, CubeSwatch, DECOR_GLYPH, heldRing } from "./cube-swatch.tsx";
import { LABEL } from "@platform/palette.ts";

/** The catalog's cubes in sections, one per cube channel, where the strip's
 * few are one flat row: too many to read without headers. Blocks first, the
 * most used, then the ground channels coarse to fine, as a place reads: what
 * the ground is, then what is on it. The order is the drawer's, a decision
 * about this list. */
const SECTION_ORDER: readonly ChannelName[] = ["block", "land", "water", "structure", "transport"];

const SECTIONS = new Map<ChannelName, PlaceableCube[]>();
for (const cube of placeableCubes) {
  const section = placeableCubeChannel(cube);
  SECTIONS.set(section, [...(SECTIONS.get(section) ?? []), cube]);
}

// A catalog entry's three sizes (issue/20260928-three-sizes-of-a-control.md):
// a press lands on its 44 cell; the tile one sees is a block's top as a
// rounded square, where a circle cut its texture away; a decor's glyph is 20.
const CELL = chrome.target;
const GRID_GAP = 4;
/** In hand, a tile grows and is ringed: 1 of the ground, then the ink. */
const LIT_SCALE = 1.05;
const LIT_INK = 2;
/** As large as a tile can be while, lit, it reaches no further than halfway
 * into the gap to the next cell: 40. */
const TILE = (CELL + GRID_GAP - 2 * (1 + LIT_INK)) / LIT_SCALE;
const TILE_RADIUS = 12;

/** A catalog entry: its picture alone, lit while it is in hand. Its name is
 * read out, and shown by its section once it is the one in hand; a grid of
 * names under every tile read as a list, where the tiles tell most apart. */
function GridButton({ label, active, onPress, children }: { label: string; active: boolean; onPress: () => void; children: React.ReactNode }) {
  return (
    <Pressable onPress={onPress} accessibilityRole="button" accessibilityLabel={label} aria-selected={active} style={styles.cell}>
      {children}
    </Pressable>
  );
}

/** A section's title, and the name of what is in hand when it is one of its
 * own; nothing, with neither, rather than an empty line over the grid. */
function SectionHead({ title, held }: { title?: string; held: string | null }) {
  if (title === undefined && held === null) return null;
  return (
    <View style={styles.sectionHead}>
      <Text style={styles.sectionTitle}>{title}</Text>
      {held !== null && <Text numberOfLines={1} style={styles.held}>{held}</Text>}
    </View>
  );
}

function CubeSwatchButton({ cube, onPick }: { cube: PlaceableCube; onPick: (cube: PlaceableCube) => void }) {
  const { t } = useTranslation();
  const active = useInHand({ tool: ActiveTool.Object, cube });

  return (
    <GridButton label={cubeLabel(t, cube)} active={active} onPress={() => onPick(cube)}>
      <CubeSwatch cube={cube} size={TILE} radius={TILE_RADIUS} style={active ? styles.markOn : styles.markOff} />
    </GridButton>
  );
}

function DecorGridButton({ kind, onPick }: { kind: u8; onPick: (kind: u8) => void }) {
  const { t } = useTranslation();
  const active = useInHand({ tool: ActiveTool.Decor, kind });
  const name = decorName(kind);

  return (
    <GridButton label={t(`decors.${name}`)} active={active} onPress={() => onPick(kind)}>
      <View style={[styles.decor, active ? [styles.markOn, styles.decorOn] : styles.markOff]}>
        <Glyph glyph={DECOR_GLYPH[name]} size={chrome.glyph} color={LABEL.primary} />
      </View>
    </GridButton>
  );
}

/** The catalog's three tabs. */
const enum DrawerTab {
  Cube = 0,
  Decor = 1,
  Character = 2,
}

/** The whole catalog, as Minecraft's inventory is to the strip's hotbar. A
 * cube or decor picked here is taken in hand as a strip pick is, and the
 * sheet closes. The character tab dresses the companion rather than placing
 * anything, so its picks apply at once and leave the sheet open
 * (CharacterPicker). */
export function DrawerModal(props: BaseModalProps) {
  const { onClose } = props;
  const { t } = useTranslation();
  const { bottom } = useSafeArea();
  const [tab, setTab] = useState<DrawerTab>(DrawerTab.Cube);
  const selectCube = useUiStore.getState().selectCube;
  const selectDecor = useUiStore.getState().selectDecor;
  const activeTool = useUiStore((s) => s.activeTool);
  const heldCube = useUiStore((s) => s.cube);
  const heldDecor = useUiStore((s) => s.decor);
  const heldSection = activeTool === ActiveTool.Object ? placeableCubeChannel(heldCube) : null;

  function pickCube(cube: PlaceableCube): void {
    selectCube(cube);
    onClose();
  }

  function pickDecor(kind: u8): void {
    selectDecor(kind);
    onClose();
  }

  return (
    <Panel label={t("drawer_modal.title")} onClose={onClose} half>
      <PanelBody
        header={<PanelHeader title={t("drawer_modal.title")} closeLabel={t("drawer_modal.close")} onClose={onClose} />}
        tabs={
          <Segmented
            value={tab}
            onChange={setTab}
            segments={[
              { value: DrawerTab.Cube, label: t("drawer_modal.tabs.cube") },
              { value: DrawerTab.Decor, label: t("drawer_modal.tabs.decor") },
              { value: DrawerTab.Character, label: t("drawer_modal.tabs.character") },
            ]}
          />
        }
      >
        <ScrollView key={tab} style={styles.scroll} contentContainerStyle={[styles.scrollContent, { paddingBottom: bottom + PANEL_EDGE }]}>
          {tab === DrawerTab.Cube && SECTION_ORDER.map((section) => {
            const cubes = SECTIONS.get(section);
            if (!cubes) return null;
            return (
              <View key={section} style={styles.section}>
                <SectionHead title={t(`drawer_modal.sections.${section}`)} held={section === heldSection ? cubeLabel(t, heldCube) : null} />
                <View style={styles.grid}>
                  {cubes.map((cube) => <CubeSwatchButton key={placeableCubeId(cube)} cube={cube} onPick={pickCube} />)}
                </View>
              </View>
            );
          })}

          {tab === DrawerTab.Decor && (
            <View style={styles.section}>
              <SectionHead held={activeTool === ActiveTool.Decor ? t(`decors.${decorName(heldDecor)}`) : null} />
              <View style={styles.grid}>
                {decorKinds.map((kind) => <DecorGridButton key={kind} kind={kind} onPick={pickDecor} />)}
              </View>
            </View>
          )}

          {tab === DrawerTab.Character && <CharacterPicker />}
        </ScrollView>
      </PanelBody>
    </Panel>
  );
}

const styles = StyleSheet.create({
  scroll: { flex: 1 },
  scrollContent: { paddingHorizontal: PANEL_EDGE, paddingTop: PANEL_TABS_ROOM, gap: PANEL_SECTION_GAP },
  section: { gap: 8 },
  sectionHead: { flexDirection: "row", alignItems: "baseline", gap: 8 },
  sectionTitle: { ...sectionTitle, color: LABEL.secondary },
  held: { flex: 1, textAlign: "right", ...textStyle.caption, color: LABEL.primary },
  grid: { flexDirection: "row", flexWrap: "wrap", gap: GRID_GAP },
  cell: { width: CELL, height: CELL, alignItems: "center", justifyContent: "center" },
  markOff: { boxShadow: `0 0 0 1px ${hairline}` },
  markOn: { boxShadow: heldRing(color.ground, color.black, LIT_INK), transform: [{ scale: LIT_SCALE }] },
  decor: { width: TILE, height: TILE, borderRadius: TILE_RADIUS, alignItems: "center", justifyContent: "center" },
  decorOn: { backgroundColor: color.faint },
});
