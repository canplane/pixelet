// @pxl/client/shell/drawer/modal.tsx

import type { ReactNode } from "react";
import { useTranslation } from "react-i18next";
import { StyleSheet, View } from "react-native";

import type { f64 } from "@xpute/kit/abi/word.ts";
import {
  allCubes,
  type AssetId,
  type Cube,
  cubeLabel,
  cubeSection,
  decorIds,
  decorLabel,
  decorSection,
  MARKER,
} from "@/shell/catalog/asset/mod.ts";
import { PIN_LABEL } from "@/shell/catalog/asset/marker.spec.ts";

import { selectAsset, useInHand } from "@/shell/world/dock/store.ts";
import { Panel } from "@/shell/ui/panel.tsx";
import { Tabbed } from "@/shell/ui/tabbed.tsx";
import { Glyph } from "@/shell/ui/glyph.tsx";
import { chrome, color } from "@/shell/ui/theme.ts";
import { SectionTitle } from "@/shell/ui/section-title.tsx";
import { CharacterPicker } from "@/shell/drawer/character/picker.tsx";
import { CubeSwatch, decorGlyph } from "@/shell/catalog/asset/swatch.tsx";
import type { GlyphName } from "@/shell/ui/icon.tsx";
import { LABEL } from "@/shell/ui/palette.ts";
import { Grid, Item, itemOf, mark } from "./grid.tsx";
import { NameTipLayer } from "./name-tip.tsx";

/** Each section's items, in the catalog's order. */
function sectioned<S, T>(
  items: readonly T[],
  sectionOf: (item: T) => S,
): Map<S, T[]> {
  const sections = new Map<S, T[]>();
  for (const item of items) {
    sections.set(sectionOf(item), [
      ...(sections.get(sectionOf(item)) ?? []),
      item,
    ]);
  }
  return sections;
}
const SECTIONS = sectioned(allCubes, cubeSection);
const DECOR_SECTIONS = sectioned(decorIds, decorSection);

// A circle cut a block's texture away, so a rounded square.
const ITEM_RADIUS = 12;

function CubeItem({
  cube,
  cell,
  onPick,
}: {
  cube: Cube;
  cell: f64;
  onPick: (cube: Cube) => void;
}) {
  const active = useInHand(cube);

  return (
    <Item
      label={cubeLabel(cube)}
      active={active}
      cell={cell}
      onPress={() => onPick(cube)}
    >
      <CubeSwatch
        cube={cube}
        size={itemOf(cell)}
        radius={ITEM_RADIUS}
        style={active ? mark.on : mark.off}
      />
    </Item>
  );
}

/** An asset drawn as its glyph: a decor, or a marker. */
function GlyphItem({
  id,
  label,
  glyph,
  cell,
  onPick,
}: {
  id: AssetId;
  label: string;
  glyph: GlyphName;
  cell: f64;
  onPick: (id: AssetId) => void;
}) {
  const active = useInHand(id);

  return (
    <Item label={label} active={active} cell={cell} onPress={() => onPick(id)}>
      <View
        style={[
          styles.decor,
          { width: itemOf(cell), height: itemOf(cell) },
          active ? [mark.on, styles.decorOn] : mark.off,
        ]}
      >
        <Glyph glyph={glyph} size={chrome.glyph} color={LABEL.primary} />
      </View>
    </Item>
  );
}

/** A tab's sections, each a grid of its items under the section's title. */
function Sections<S extends string, T>({
  kind,
  sections,
  item,
}: {
  kind: "cube" | "decor";
  sections: Map<S, T[]>;
  item: (it: T, cell: f64) => ReactNode;
}) {
  const { t } = useTranslation();
  return [...sections].map(([section, items]) => (
    <Grid
      key={section}
      head={
        <SectionTitle>{t(`drawer.sections.${kind}.${section}`)}</SectionTitle>
      }
    >
      {(cell) => items.map((it) => item(it, cell))}
    </Grid>
  ));
}

const CUBE_TAB = 0;
const DECOR_TAB = 1;
/** A temporary way in: one marker, its words not kept yet. */
const MARKER_TAB = 2;
const CHARACTER_TAB = 3;

/** Cube, decor and marker picks close the sheet; character picks apply at once and leave it open. */
export function DrawerModal({ onClose }: { onClose: () => void }) {
  const { t } = useTranslation();

  function pick(id: AssetId): void {
    selectAsset(id);
    onClose();
  }

  return (
    <Panel label={t("drawer.title")} onClose={onClose} half>
      <NameTipLayer>
        <Tabbed
          onClose={onClose}
          tabs={[
            { value: CUBE_TAB, label: t("drawer.tabs.cube") },
            { value: DECOR_TAB, label: t("drawer.tabs.decor") },
            { value: MARKER_TAB, label: t("drawer.tabs.marker") },
            { value: CHARACTER_TAB, label: t("drawer.tabs.character") },
          ]}
        >
          {(tab) => (
            <>
              {tab === CUBE_TAB && (
                <Sections
                  kind="cube"
                  sections={SECTIONS}
                  item={(cube, cell) => (
                    <CubeItem
                      key={cube}
                      cube={cube}
                      cell={cell}
                      onPick={pick}
                    />
                  )}
                />
              )}

              {tab === DECOR_TAB && (
                <Sections
                  kind="decor"
                  sections={DECOR_SECTIONS}
                  item={(kind, cell) => (
                    <GlyphItem
                      key={kind}
                      id={kind}
                      label={decorLabel(kind)}
                      glyph={decorGlyph(kind)}
                      cell={cell}
                      onPick={pick}
                    />
                  )}
                />
              )}

              {tab === MARKER_TAB && (
                <Grid
                  head={<SectionTitle>{t("drawer.sections.pin")}</SectionTitle>}
                >
                  {(cell) => (
                    <GlyphItem
                      id={MARKER}
                      label={PIN_LABEL}
                      glyph="bubble.left"
                      cell={cell}
                      onPick={pick}
                    />
                  )}
                </Grid>
              )}

              {tab === CHARACTER_TAB && <CharacterPicker />}
            </>
          )}
        </Tabbed>
      </NameTipLayer>
    </Panel>
  );
}

const styles = StyleSheet.create({
  decor: {
    borderRadius: ITEM_RADIUS,
    alignItems: "center",
    justifyContent: "center",
  },
  decorOn: { backgroundColor: color.faint },
});
