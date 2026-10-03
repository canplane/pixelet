// @pxl/web/feature/drawer/modal.tsx

import { useState } from "react";
import { useTranslation } from "react-i18next";
import { Pressable, StyleSheet, Text, View } from "react-native";

import type { u8 } from "@xpute/core/abi/word.ts";
import { type ChannelName, decorKinds, type PlaceableCube, placeableCubeChannel, placeableCubeId, placeableCubes } from "@/catalog/cell/mod.ts";

import { useInHand, useUiStore } from "@/app/store/ui.ts";
import { ActiveTool } from "@/kernel/abi/cmd.spec.ts";
import { Panel } from "@platform/panel.tsx";
import { Glyph } from "@/shared/ui/glyph.tsx";
import { PanelBody, PanelHeader, PanelScroll } from "@/shared/ui/panel.tsx";
import { Segmented } from "@platform/segmented.tsx";
import { chrome, color, hairline, sectionTitle, textStyle } from "@/shared/ui/theme.ts";
import { CharacterPicker } from "@/feature/character/picker.tsx";
import { cubeLabel, CubeSwatch, decorGlyph, decorLabel, heldRing } from "@/catalog/cell/swatch.tsx";
import { LABEL } from "@platform/palette.ts";

/** In the catalog's order. */
const SECTIONS = new Map<ChannelName, PlaceableCube[]>();
for (const cube of placeableCubes) {
  const section = placeableCubeChannel(cube);
  SECTIONS.set(section, [...(SECTIONS.get(section) ?? []), cube]);
}

// A press lands on 44 (issue/20260928-three-sizes-of-a-control.md); a circle cut a block's texture away, so a rounded square.
const GRID_GAP = 4;
const LIT_SCALE = 1.05;
const LIT_INK = 2;
/** Lit, a tile reaches no further than halfway into the gap: 40. */
const TILE = (chrome.target + GRID_GAP - 2 * (1 + LIT_INK)) / LIT_SCALE;
const TILE_RADIUS = 12;

/** Pictures only: names under every tile read as a list. */
function GridButton({ label, active, onPress, children }: { label: string; active: boolean; onPress: () => void; children: React.ReactNode }) {
  return (
    <Pressable onPress={onPress} accessibilityRole="button" accessibilityLabel={label} aria-selected={active} style={styles.cell}>
      {children}
    </Pressable>
  );
}

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

  return (
    <GridButton label={decorLabel(t, kind)} active={active} onPress={() => onPick(kind)}>
      <View style={[styles.decor, active ? [styles.markOn, styles.decorOn] : styles.markOff]}>
        <Glyph glyph={decorGlyph(kind)} size={chrome.glyph} color={LABEL.primary} />
      </View>
    </GridButton>
  );
}

const enum DrawerTab {
  Cube = 0,
  Decor = 1,
  Character = 2,
}

/** Cube and decor picks close the sheet; character picks apply at once and leave it open. */
export function DrawerModal({ onClose }: { onClose: () => void }) {
  const { t } = useTranslation();
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
    <Panel label={t("drawer.title")} onClose={onClose} half>
      <PanelBody
        header={<PanelHeader title={t("drawer.title")} onClose={onClose} />}
        tabs={
          <Segmented
            value={tab}
            onChange={setTab}
            segments={[
              { value: DrawerTab.Cube, label: t("drawer.tabs.cube") },
              { value: DrawerTab.Decor, label: t("drawer.tabs.decor") },
              { value: DrawerTab.Character, label: t("drawer.tabs.character") },
            ]}
          />
        }
      >
        <PanelScroll key={tab}>
          {tab === DrawerTab.Cube && [...SECTIONS].map(([section, cubes]) => {
            return (
              <View key={section} style={styles.section}>
                <SectionHead title={t(`drawer.sections.${section}`)} held={section === heldSection ? cubeLabel(t, heldCube) : null} />
                <View style={styles.grid}>
                  {cubes.map((cube) => <CubeSwatchButton key={placeableCubeId(cube)} cube={cube} onPick={pickCube} />)}
                </View>
              </View>
            );
          })}

          {tab === DrawerTab.Decor && (
            <View style={styles.section}>
              <SectionHead held={activeTool === ActiveTool.Decor ? decorLabel(t, heldDecor) : null} />
              <View style={styles.grid}>
                {decorKinds.map((kind) => <DecorGridButton key={kind} kind={kind} onPick={pickDecor} />)}
              </View>
            </View>
          )}

          {tab === DrawerTab.Character && <CharacterPicker />}
        </PanelScroll>
      </PanelBody>
    </Panel>
  );
}

const styles = StyleSheet.create({
  section: { gap: 8 },
  sectionHead: { flexDirection: "row", alignItems: "baseline", gap: 8 },
  sectionTitle: { ...sectionTitle, color: LABEL.secondary },
  held: { flex: 1, textAlign: "right", ...textStyle.caption, color: LABEL.primary },
  grid: { flexDirection: "row", flexWrap: "wrap", gap: GRID_GAP },
  cell: { width: chrome.target, height: chrome.target, alignItems: "center", justifyContent: "center" },
  markOff: { boxShadow: `0 0 0 1px ${hairline}` },
  markOn: { boxShadow: heldRing(color.ground, color.black, LIT_INK), transform: [{ scale: LIT_SCALE }] },
  decor: { width: TILE, height: TILE, borderRadius: TILE_RADIUS, alignItems: "center", justifyContent: "center" },
  decorOn: { backgroundColor: color.faint },
});
