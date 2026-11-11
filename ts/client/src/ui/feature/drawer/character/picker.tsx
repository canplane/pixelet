// @pxl/client/ui/feature/drawer/character/picker.tsx

import { type ReactNode, useState } from "react";
import { useTranslation } from "react-i18next";
import { Pressable, StyleSheet, Text, View } from "react-native";

import {
  color,
  hairline,
  textScale,
  textStyle,
  weight,
} from "@/ui/shared/component/theme.ts";
import { SectionTitle } from "@/ui/shared/component/section-title.tsx";
import { PickedCharacter } from "./picked.tsx";
import { SETTING } from "@/ui/shared/setting.ts";
import {
  type Character,
  type Gender,
  genderOf,
  GENDER_LABEL_KEY,
  GENDERS,
  personAt,
  personsOf,
  PETS,
  petAt,
} from "./character.ts";
import { LABEL } from "@/ui/shared/component/palette.ts";
import { PANEL_EDGE, PANEL_SECTION_GAP } from "@/ui/shared/component/panel.tsx";
import { useCore } from "@/ui/shared/watch.ts";
import { touchOutset } from "@/ui/shared/component/touch.ts";
import * as setting from "@/service/core/setting.ts";

const CHIP_HEIGHT = 32;
const SURFACE_SIZE = 96;
const CHIP_GAP_X = 8;
const CHIP_GAP_Y = 2 * touchOutset(CHIP_HEIGHT);
const CHIP_TARGET = {
  paddingHorizontal: CHIP_GAP_X / 2,
  paddingVertical: touchOutset(CHIP_HEIGHT),
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
      <View style={[styles.chip, active && styles.chipOn]}>
        {/* A hidden copy in the heavier weight sizes the chip, so a pick never rewraps the row. */}
        <Text
          aria-hidden
          maxFontSizeMultiplier={textScale.chrome}
          style={[styles.chipText, styles.chipTextOn, styles.sizer]}
        >
          {label}
        </Text>
        <View style={styles.label}>
          <Text
            maxFontSizeMultiplier={textScale.chrome}
            style={[
              styles.chipText,
              styles.centered,
              active && styles.chipTextOn,
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
    <View style={styles.picker}>
      <SectionTitle>{label}</SectionTitle>
      <View style={styles.pickerRow}>
        <PickedCharacter character={selected} style={styles.surface} />
        <View style={styles.chipColumn}>{children}</View>
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
  const characters = personsOf(gender);

  return (
    <PickerCard label={t("drawer.character.person_label")} selected={selected}>
      <View style={styles.chips}>
        {GENDERS.map((g) => (
          <Chip
            key={g}
            label={t(GENDER_LABEL_KEY[g])}
            active={gender === g}
            onPress={() => onGenderChange(g)}
          />
        ))}
      </View>
      <View style={styles.rule} />
      <View style={styles.chips}>
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
      <View style={styles.chips}>
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
  const person = personAt(useCore((s) => s.settings[SETTING.person.key]));
  const pet = petAt(useCore((s) => s.settings[SETTING.pet.key]));

  // The picker opens on the selected person's gender, so that a pick does not
  // look reset when the sheet opens again.
  const [gender, setGender] = useState<Gender>(genderOf(person));

  return (
    <View style={styles.pickers}>
      <PersonPicker
        gender={gender}
        onGenderChange={setGender}
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

const styles = StyleSheet.create({
  pickers: { gap: PANEL_SECTION_GAP },
  picker: { gap: 8 },
  pickerRow: {
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
    backgroundColor: color.backdrop,
  },
  chipColumn: {
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
    backgroundColor: hairline,
  },
  chip: {
    height: CHIP_HEIGHT,
    justifyContent: "center",
    borderRadius: CHIP_HEIGHT / 2,
    paddingHorizontal: 12,
    backgroundColor: color.faint,
  },
  chipOn: { backgroundColor: color.chosen },
  chipText: {
    ...textStyle.footnote,
    fontWeight: weight.medium,
    color: LABEL.primary,
  },
  chipTextOn: { fontWeight: weight.semibold, color: color.ground },
  sizer: { opacity: 0 },
  label: { ...StyleSheet.absoluteFill, justifyContent: "center" },
  centered: { textAlign: "center" },
});
