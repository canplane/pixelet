// @pxl/web/feature/map/ui/overlay/drawer-modal.tsx

import { useState } from "react";
import { useTranslation } from "react-i18next";
import { Pressable, ScrollView, StyleSheet, Text, View } from "react-native";

import type { u8 } from "@xpute/core/abi/word.ts";
import { type ChannelName, decor_kinds, decorName, placeable_cubes, type PlaceableCube, placeableCubeChannel, placeableCubeId, sameCube } from "@/catalog/cell/mod.ts";

import type { BaseModalProps } from "@/app/type/ui.type.ts";
import { useUiStore } from "@/app/store/ui-store.ts";
import { ActiveTool } from "@/kernel/abi/cmd.spec.ts";
import { useSafeArea } from "@platform/safe-area.ts";
import { Panel } from "@platform/panel.tsx";
import { PANEL_EDGE, PANEL_TABS_ROOM, PanelBody, PanelHeader } from "@/shared/ui/panel.tsx";
import { Segmented } from "@platform/segmented.tsx";
import { color, hairline, sectionTitle, textStyle } from "@/shared/ui/theme.ts";
import { CharacterPicker } from "@/feature/character/picker.tsx";
import { CubeSwatch } from "./cube-swatch.tsx";
import { DECOR_ICONS } from "./mode-bar.tsx";
import { LABEL } from "@platform/palette.ts";

/** The full catalog's own grouping — mode-bar.tsx's hotbar stays a flat
 * scrolling strip (no sections, that's the point of a hotbar), but a grid
 * with 30+ terrain-styled entries alongside the original 15 colors reads as
 * noise without headers. A section is a cube channel and nothing else, block
 * first — the original, most-used set — then the four ground channels coarse
 * to fine, which is how a place reads: what the ground is, then what is on it.
 *
 * The order is the drawer's own. A channel's name comes from the catalog, but
 * which order a palette shows them in is a decision about this list and
 * belongs beside it. */
const SECTION_ORDER: readonly ChannelName[] = ["block", "land", "water", "structure", "transport"];

function groupBySection(cubes: readonly PlaceableCube[]): Map<ChannelName, PlaceableCube[]> {
  const out = new Map<ChannelName, PlaceableCube[]>();
  for (const cube of cubes) {
    const key = placeableCubeChannel(cube);
    const list = out.get(key);
    if (list) list.push(cube);
    else out.set(key, [cube]);
  }
  return out;
}

/** A cube's words are `"<channel>.<type>"` (cell/mod.ts), e.g.
 * "land.green_deep" or "block.white" — i18next's own default "."
 * keySeparator resolves that straight into the nested
 * `cubes.land.green_deep` locale entry (en.json/ko.json). */
function cubeLabel(t: (key: string) => string, cube: PlaceableCube): string {
  return t(`cubes.${placeableCubeId(cube)}`);
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
  const activeTool = useUiStore((s) => s.activeTool);
  const held = useUiStore((s) => s.cube);
  const active = activeTool === ActiveTool.Object && sameCube(held, cube);

  return (
    <GridButton label={cubeLabel(t, cube)} active={active} onPress={() => onPick(cube)}>
      <CubeSwatch cube={cube} size={32} radius={16} style={active ? styles.markOn : styles.markOff} />
    </GridButton>
  );
}

function DecorGridButton({ kind, onPick }: { kind: u8; onPick: (kind: u8) => void }) {
  const { t } = useTranslation();
  const activeTool = useUiStore((s) => s.activeTool);
  const held = useUiStore((s) => s.decor);
  const active = activeTool === ActiveTool.Decor && held === kind;
  const name = decorName(kind);
  const Icon = DECOR_ICONS[name];

  return (
    <GridButton label={t(`decors.${name}`)} active={active} onPress={() => onPick(kind)}>
      <View style={[styles.decor, active ? [styles.markOn, styles.decorOn] : styles.markOff]}>
        <Icon size={16} strokeWidth={2} color={LABEL.primary} />
      </View>
    </GridButton>
  );
}

/** The catalog's three tabs. */
export const enum DrawerTab {
  Cube = 0,
  Decor = 1,
  Character = 2,
}

/** The full cube/decor/character catalog: the Minecraft-inventory-style
 * "everything" view beside mode-bar.tsx's hotbar, which is the everyday
 * quick-pick, opened from its own trigger button. Picking a Cube/Decor entry
 * sets what a hotbar pick sets (the same ui-store fields), then closes. The Character tab is
 * the odd one out: it equips the single always-on companion rather than
 * placing anything, so its picks (CharacterPicker) apply immediately and
 * deliberately leave the sheet open — see that component's own comment. */
export function DrawerModal(props: BaseModalProps) {
  const { onClose } = props;
  const { t } = useTranslation();
  const { bottom } = useSafeArea();
  const [tab, setTab] = useState<DrawerTab>(DrawerTab.Cube);
  const selectCube = useUiStore.getState().selectCube;
  const selectDecor = useUiStore.getState().selectDecor;

  const sections = groupBySection(placeable_cubes);

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
            const cubes = sections.get(section);
            if (!cubes || cubes.length === 0) return null;
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
              {decor_kinds.map((kind) => <DecorGridButton key={kind} kind={kind} onPick={pickDecor} />)}
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
  // Five to a row, as the catalog was laid out.
  grid: { flexDirection: "row", flexWrap: "wrap", gap: 4 },
  cell: { width: "18.4%", alignItems: "center", gap: 4, padding: 6, borderRadius: 20 },
  cellLabel: { width: "100%", textAlign: "center", ...textStyle.caption2, color: LABEL.secondary },
  markOff: { boxShadow: `0 0 0 1px ${hairline}` },
  markOn: { boxShadow: `0 0 0 1px ${color.paper}, 0 0 0 3px ${color.black}`, transform: [{ scale: 1.05 }] },
  decor: { width: 32, height: 32, borderRadius: 16, alignItems: "center", justifyContent: "center" },
  decorOn: { backgroundColor: color.faint },
});
