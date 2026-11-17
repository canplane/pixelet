// @pxl/client/ui/feature/drawer/modal.tsx

import type { ReactNode } from "react";
import { useTranslation } from "react-i18next";
import { StyleSheet, View } from "react-native";

import type { f64 } from "@xpute/kit/abi/word.ts";
import {
  type AssetId,
  type Cube,
  CUBES,
  cubeLabel,
  cubeSection,
  decorLabel,
  decorSection,
  MARKER,
} from "@/ui/shared/asset/catalog.ts";
import { DECORS } from "@/ui/shared/asset/decor.spec.ts";
import { PIN_LABEL } from "@/ui/shared/asset/marker.spec.ts";

import { selectAsset, useInHand } from "@/ui/feature/world/dock/store.ts";
import { Panel } from "@/ui/shared/component/panel.tsx";
import { Tabbed } from "@/ui/shared/component/tabbed.tsx";
import { Glyph } from "@/ui/shared/component/glyph.tsx";
import { CHROME, COLOR } from "@/ui/shared/component/theme.ts";
import { SectionTitle } from "@/ui/shared/component/section-title.tsx";
import { CharacterPicker } from "@/ui/feature/drawer/character/picker.tsx";
import { CubeSwatch, decorGlyph } from "@/ui/shared/asset/swatch.tsx";
import type { GlyphName } from "@/ui/shared/component/icon.tsx";
import { LABEL } from "@/ui/shared/component/palette.ts";
import { Grid, Item, itemOf, MARK } from "./grid.tsx";
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
const SECTIONS = sectioned(CUBES, cubeSection);
const DECOR_SECTIONS = sectioned(DECORS, decorSection);

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
        style={active ? MARK.on : MARK.off}
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
          STYLE.decor,
          { width: itemOf(cell), height: itemOf(cell) },
          active ? [MARK.on, STYLE.decorOn] : MARK.off,
        ]}
      >
        <Glyph glyph={glyph} size={CHROME.glyph} color={LABEL.primary} />
      </View>
    </Item>
  );
}

function Sections<T>({
  kind,
  sections,
  item,
}: {
  kind: "cube" | "decor";
  sections: Map<string, T[]>;
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
                  item={(id, cell) => (
                    <GlyphItem
                      key={id}
                      id={id}
                      label={decorLabel(id)}
                      glyph={decorGlyph(id)}
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

const STYLE = StyleSheet.create({
  decor: {
    borderRadius: ITEM_RADIUS,
    alignItems: "center",
    justifyContent: "center",
  },
  decorOn: { backgroundColor: COLOR.faint },
});
