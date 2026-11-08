// @pxl/client/ui/drawer/modal.tsx

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
} from "@/ui/asset/catalog.ts";
import { PIN_LABEL } from "@/ui/asset/marker.spec.ts";

import { selectAsset, useInHand } from "@/ui/world/dock/store.ts";
import { Panel } from "@/ui/component/panel.tsx";
import { Tabbed } from "@/ui/component/tabbed.tsx";
import { Glyph } from "@/ui/component/glyph.tsx";
import { chrome, color } from "@/ui/component/theme.ts";
import { SectionTitle } from "@/ui/component/section-title.tsx";
import { CharacterPicker } from "@/ui/drawer/character/picker.tsx";
import { CubeSwatch, decorGlyph } from "@/ui/asset/swatch.tsx";
import type { GlyphName } from "@/ui/component/icon.tsx";
import { LABEL } from "@/ui/component/palette.ts";
import { Grid, Item, itemOf, mark } from "./grid.tsx";
import { NameTipLayer } from "./name-tip.tsx";

/** The sections, and the items in each, keep the catalog's order. */
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

// An item is a rounded square: a circle would cut away too much of a block's
// texture.
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
const MARKER_TAB = 2;
const CHARACTER_TAB = 3;

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
