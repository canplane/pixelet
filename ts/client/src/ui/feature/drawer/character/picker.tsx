// @pxl/client/ui/feature/drawer/character/picker.tsx

import { type ReactNode, useState } from "react";
import { useTranslation } from "react-i18next";
import { Pressable, StyleSheet, Text, View } from "react-native";

import {
  COLOR,
  HAIRLINE,
  TEXT_SCALE_CAP,
  TEXT_STYLE,
  WEIGHT,
} from "@/ui/shared/component/theme.ts";
import { SectionTitle } from "@/ui/shared/component/section-title.tsx";
import { PickedCharacter } from "./picked.tsx";
import { SETTING } from "@/ui/shared/setting.ts";
import {
  type Character,
  gender_of,
  GENDER_LABEL_KEY,
  GENDERS,
  person_at,
  persons_of,
  PETS,
  pet_at,
} from "./character.ts";
import type { Gender } from "@/ui/shared/asset/character.spec.ts";
import { LABEL } from "@/ui/shared/component/palette.ts";
import { PANEL_EDGE, PANEL_SECTION_GAP } from "@/ui/shared/component/panel.tsx";
import { useCore } from "@/ui/shared/watch.ts";
import { touch_outset } from "@/ui/shared/component/touch.ts";
import * as setting from "@/service/core/setting.ts";

const CHIP_HEIGHT = 32;
const SURFACE_SIZE = 96;
const CHIP_GAP_X = 8;
const CHIP_GAP_Y = 2 * touch_outset(CHIP_HEIGHT);
const CHIP_TARGET = {
  paddingHorizontal: CHIP_GAP_X / 2,
  paddingVertical: touch_outset(CHIP_HEIGHT),
};

function Chip({
  label,
  active,
  onPress,
}: {
  label: string;
  active: boolean;
  onPress: () => void;
}) {
  return (
    <Pressable
      onPress={onPress}
      accessibilityRole="radio"
      aria-checked={active}
      style={CHIP_TARGET}
    >
      <View style={[STYLE.chip, active && STYLE.chip_on]}>
        {/* A hidden copy in the heavier weight sizes the chip, so a pick never rewraps the row. */}
        <Text
          aria-hidden
          maxFontSizeMultiplier={TEXT_SCALE_CAP.chrome}
          style={[STYLE.chip_text, STYLE.chip_text_on, STYLE.sizer]}
        >
          {label}
        </Text>
        <View style={STYLE.label}>
          <Text
            maxFontSizeMultiplier={TEXT_SCALE_CAP.chrome}
            style={[
              STYLE.chip_text,
              STYLE.centered,
              active && STYLE.chip_text_on,
            ]}
          >
            {label}
          </Text>
        </View>
      </View>
    </Pressable>
  );
}

function PickerCard({
  label,
  selected,
  children,
}: {
  label: string;
  selected: Character;
  children: ReactNode;
}) {
  return (
    <View style={STYLE.picker}>
      <SectionTitle>{label}</SectionTitle>
      <View style={STYLE.picker_row}>
        <PickedCharacter character={selected} style={STYLE.surface} />
        <View style={STYLE.chip_column}>{children}</View>
      </View>
    </View>
  );
}

interface PersonPickerProps {
  gender: Gender;
  onGenderChange: (gender: Gender) => void;
  selected: Character;
  onSelect: (character: Character) => void;
}

/** Switching between genders changes nothing until a character is picked. */
function PersonPicker(props: PersonPickerProps) {
  const { gender, onGenderChange, selected, onSelect } = props;
  const { t } = useTranslation();
  const characters = persons_of(gender);

  return (
    <PickerCard label={t("drawer.character.person_label")} selected={selected}>
      <View style={STYLE.chips}>
        {GENDERS.map((g) => (
          <Chip
            key={g}
            label={t(GENDER_LABEL_KEY[g])}
            active={gender === g}
            onPress={() => onGenderChange(g)}
          />
        ))}
      </View>
      <View style={STYLE.rule} />
      <View style={STYLE.chips}>
        {characters.map((v, i) => (
          <Chip
            key={v.number}
            label={`${i + 1}`}
            active={v.number === selected.number}
            onPress={() => onSelect(v)}
          />
        ))}
      </View>
    </PickerCard>
  );
}

interface PetPickerProps {
  selected: Character;
  onSelect: (character: Character) => void;
}

function PetPicker(props: PetPickerProps) {
  const { selected, onSelect } = props;
  const { t } = useTranslation();

  return (
    <PickerCard label={t("drawer.character.pet_label")} selected={selected}>
      <View style={STYLE.chips}>
        {PETS.map((v) => (
          <Chip
            key={v.number}
            label={v.label}
            active={v.number === selected.number}
            onPress={() => onSelect(v)}
          />
        ))}
      </View>
    </PickerCard>
  );
}

/** A pick takes effect at once and the sheet stays open, so that characters
 * can be compared. */
export function CharacterPicker() {
  const person = person_at(useCore((s) => s.settings[SETTING.person.key]));
  const pet = pet_at(useCore((s) => s.settings[SETTING.pet.key]));

  // The picker opens on the selected person's gender, so that a pick does not
  // look reset when the sheet opens again.
  const [gender, set_gender] = useState<Gender>(gender_of(person));

  return (
    <View style={STYLE.pickers}>
      <PersonPicker
        gender={gender}
        onGenderChange={set_gender}
        selected={person}
        onSelect={(v) => setting.set(SETTING.person.key, v.number)}
      />
      <PetPicker
        selected={pet}
        onSelect={(v) => setting.set(SETTING.pet.key, v.number)}
      />
    </View>
  );
}

const STYLE = StyleSheet.create({
  pickers: { gap: PANEL_SECTION_GAP },
  picker: { gap: 8 },
  picker_row: {
    flexDirection: "row",
    alignItems: "flex-start",
    gap: PANEL_EDGE,
  },
  surface: {
    width: SURFACE_SIZE,
    height: SURFACE_SIZE,
    flexShrink: 0,
    borderRadius: 12,
    overflow: "hidden",
    backgroundColor: COLOR.backdrop,
  },
  chip_column: {
    flexShrink: 1,
    minWidth: 0,
    gap: CHIP_GAP_Y / 2,
    marginHorizontal: -CHIP_GAP_X / 2,
    marginVertical: -CHIP_GAP_Y / 2,
  },
  chips: { flexDirection: "row", flexWrap: "wrap" },
  rule: {
    height: 1,
    marginHorizontal: CHIP_GAP_X / 2,
    backgroundColor: HAIRLINE,
  },
  chip: {
    height: CHIP_HEIGHT,
    justifyContent: "center",
    borderRadius: CHIP_HEIGHT / 2,
    paddingHorizontal: 12,
    backgroundColor: COLOR.faint,
  },
  chip_on: { backgroundColor: COLOR.chosen },
  chip_text: {
    ...TEXT_STYLE.footnote,
    fontWeight: WEIGHT.medium,
    color: LABEL.primary,
  },
  chip_text_on: { fontWeight: WEIGHT.semibold, color: COLOR.ground },
  sizer: { opacity: 0 },
  label: { ...StyleSheet.absoluteFill, justifyContent: "center" },
  centered: { textAlign: "center" },
});
