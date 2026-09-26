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
import { Panel, PanelHeader } from "@/shared/ui/panel.tsx";
import { Segmented } from "@/shared/ui/segmented.tsx";
import { alpha, color, font, hairline } from "@/shared/ui/theme.ts";
import { CharacterPicker } from "@/feature/character/picker.tsx";
import { CubeSwatch } from "./cube-swatch.tsx";
import { DECOR_ICONS } from "./mode-bar.tsx";

export interface DrawerModalProps {
  type: "drawer";
}

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
      <CubeSwatch cube={cube} size={32} radius={6} style={active ? styles.markOn : styles.markOff} />
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
        <Icon size={16} strokeWidth={2} color={color.foreground} />
      </View>
    </GridButton>
  );
}

type Tab = "cube" | "decor" | "character";

/** The full cube/decor/character catalog — mode-bar.tsx's own hotbar (its
 * horizontal scrolling color strip) stays untouched and is the everyday
 * quick-pick; this is the Minecraft-inventory-style "everything" view opened
 * from its own trigger button, never a replacement for the hotbar. Picking a
 * Cube/Decor entry does exactly what its (now-removed) hotbar counterpart
 * used to (set the same ui-store fields), then closes. The Character tab is
 * the odd one out: it equips the single always-on companion rather than
 * placing anything, so its picks (CharacterPicker) apply immediately and
 * deliberately leave the sheet open — see that component's own comment. */
export function DrawerModal(props: BaseModalProps & DrawerModalProps) {
  const { onOpenChange } = props;
  const { t } = useTranslation();
  const { bottom } = useSafeArea();
  const [tab, setTab] = useState<Tab>("cube");
  const selectCube = useUiStore.getState().selectCube;
  const selectDecor = useUiStore.getState().selectDecor;
  const close = () => onOpenChange(false);

  const sections = groupBySection(placeable_cubes);

  function pickCube(cube: PlaceableCube): void {
    selectCube(cube);
    close();
  }

  function pickDecor(kind: u8): void {
    selectDecor(kind);
    close();
  }

  return (
    <Panel side="right" width={360} label={t("drawer_modal.title")} onClose={close}>
      <PanelHeader title={t("drawer_modal.title")} closeLabel={t("drawer_modal.close")} onClose={close} />

      <View style={styles.tabs}>
        <Segmented
          value={tab}
          onChange={setTab}
          segments={[
            { value: "cube", label: t("drawer_modal.tabs.cube") },
            { value: "decor", label: t("drawer_modal.tabs.decor") },
            { value: "character", label: t("drawer_modal.tabs.character") },
          ]}
        />
      </View>

      <ScrollView key={tab} style={styles.scroll} contentContainerStyle={[styles.scrollContent, { paddingBottom: bottom + 16 }]}>
        {tab === "cube" && SECTION_ORDER.map((section) => {
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

        {tab === "decor" && (
          <View style={styles.grid}>
            {decor_kinds.map((kind) => <DecorGridButton key={kind} kind={kind} onPick={pickDecor} />)}
          </View>
        )}

        {tab === "character" && <CharacterPicker />}
      </ScrollView>
    </Panel>
  );
}

const styles = StyleSheet.create({
  tabs: { paddingHorizontal: 20, paddingTop: 16 },
  scroll: { flex: 1 },
  scrollContent: { paddingHorizontal: 20, paddingTop: 16, gap: 16 },
  section: { gap: 8 },
  sectionTitle: { fontFamily: font.family, fontSize: 12, fontWeight: "500", color: color.mutedForeground },
  // Five to a row, as the catalog was laid out.
  grid: { flexDirection: "row", flexWrap: "wrap", gap: 4 },
  cell: { width: "18.4%", alignItems: "center", gap: 4, padding: 6, borderRadius: 8 },
  cellLabel: { width: "100%", textAlign: "center", fontFamily: font.family, fontSize: 10, lineHeight: 12, color: color.mutedForeground },
  markOff: { boxShadow: `0 0 0 1px ${hairline}` },
  markOn: { boxShadow: `0 0 0 1px ${color.background}, 0 0 0 3px ${color.foreground}`, transform: [{ scale: 1.05 }] },
  decor: { width: 32, height: 32, borderRadius: 6, alignItems: "center", justifyContent: "center" },
  decorOn: { backgroundColor: alpha(color.accent, 0.6) },
});
