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
import { PANEL_EDGE, PANEL_TABS_ROOM, PanelBody, PanelHeader } from "@/shared/ui/panel.tsx";
import { Segmented } from "@platform/segmented.tsx";
import { color, hairline, sectionTitle, textStyle } from "@/shared/ui/theme.ts";
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

/** A catalog entry: a picture over its name, lit while it is in hand. */
function GridButton({ label, active, onPress, children }: { label: string; active: boolean; onPress: () => void; children: React.ReactNode }) {
  return (
    <Pressable onPress={onPress} accessibilityRole="button" accessibilityLabel={label} aria-selected={active} style={styles.cell}>
      {children}
      <Text numberOfLines={1} style={styles.cellLabel}>{label}</Text>
    </Pressable>
  );
}

function CubeSwatchButton({ cube, onPick }: { cube: PlaceableCube; onPick: (cube: PlaceableCube) => void }) {
  const { t } = useTranslation();
  const active = useInHand({ tool: ActiveTool.Object, cube });

  return (
    <GridButton label={cubeLabel(t, cube)} active={active} onPress={() => onPick(cube)}>
      <CubeSwatch cube={cube} size={32} radius={16} style={active ? styles.markOn : styles.markOff} />
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
        <Glyph glyph={DECOR_GLYPH[name]} size={16} color={LABEL.primary} />
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
        <ScrollView key={tab} style={styles.scroll} contentContainerStyle={[styles.scrollContent, { paddingBottom: bottom + 16 }]}>
          {tab === DrawerTab.Cube && SECTION_ORDER.map((section) => {
            const cubes = SECTIONS.get(section);
            if (!cubes) return null;
            return (
              <View key={section} style={styles.section}>
                <Text style={styles.sectionTitle}>{t(`drawer_modal.sections.${section}`)}</Text>
                <View style={styles.grid}>
                  {cubes.map((cube) => <CubeSwatchButton key={placeableCubeId(cube)} cube={cube} onPick={pickCube} />)}
                </View>
              </View>
            );
          })}

          {tab === DrawerTab.Decor && (
            <View style={styles.grid}>
              {decorKinds.map((kind) => <DecorGridButton key={kind} kind={kind} onPick={pickDecor} />)}
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
  scrollContent: { paddingHorizontal: PANEL_EDGE, paddingTop: PANEL_TABS_ROOM, gap: 16 },
  section: { gap: 8 },
  sectionTitle: { ...sectionTitle, color: LABEL.secondary },
  // Five to a row: a fifth of the row less a share of the four 4-point gaps.
  grid: { flexDirection: "row", flexWrap: "wrap", gap: 4 },
  cell: { width: "18.4%", alignItems: "center", gap: 4, padding: 6, borderRadius: 20 },
  cellLabel: { width: "100%", textAlign: "center", ...textStyle.caption2, color: LABEL.secondary },
  markOff: { boxShadow: `0 0 0 1px ${hairline}` },
  markOn: { boxShadow: heldRing(color.paper, color.black, 2), transform: [{ scale: 1.05 }] },
  decor: { width: 32, height: 32, borderRadius: 16, alignItems: "center", justifyContent: "center" },
  decorOn: { backgroundColor: color.faint },
});
