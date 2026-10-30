// @pxl/client/shell/drawer/character/picker.tsx

import { type ReactNode, useState } from "react";
import { useTranslation } from "react-i18next";
import { Pressable, StyleSheet, Text, View } from "react-native";

import {
  alpha,
  color,
  hairline,
  textScale,
  textStyle,
  weight,
} from "@/shell/ui/theme.ts";
import { SectionTitle } from "@/shell/ui/section-title.tsx";
import { CharacterPreview } from "./preview.tsx";
import { kernel_setting } from "@/port/shell.ts";
import { SETTING } from "@/shell/setting/schema.ts";
import {
  type CharacterVariant,
  type Gender,
  genderOf,
  GENDERS,
  personVariant,
  personVariantsOf,
  PET_VARIANTS,
  petVariant,
} from "./variant.ts";
import { LABEL } from "@/shell/ui/palette.ts";
import { PANEL_EDGE, PANEL_SECTION_GAP } from "@/shell/ui/panel.tsx";
import { useKernel } from "@/shell/store/watch.ts";
import { touchReach } from "@/shell/ui/touch.ts";

// The chip one sees is 32; a press lands on 44, a box laid out around it.
const CHIP_HEIGHT = 32;
const PREVIEW = 96;
const CHIP_GAP_X = 8;
const CHIP_GAP_Y = 2 * touchReach(CHIP_HEIGHT);
const CHIP_REACH = {
  paddingHorizontal: CHIP_GAP_X / 2,
  paddingVertical: touchReach(CHIP_HEIGHT),
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
      style={CHIP_REACH}
    >
      <View style={[styles.chip, active && styles.chipOn]}>
        {/* Sized to the label's heavier weight, so a pick never rewraps the row. */}
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

/** A preview of the one picked beside the chips that pick it. */
function PickerCard({
  label,
  selected,
  children,
}: {
  label: string;
  selected: CharacterVariant;
  children: ReactNode;
}) {
  return (
    <View style={styles.picker}>
      <SectionTitle>{label}</SectionTitle>
      <View style={styles.pickerRow}>
        <CharacterPreview variant={selected} style={styles.preview} />
        <View style={styles.chipColumn}>{children}</View>
      </View>
    </View>
  );
}

interface PersonPickerProps {
  gender: Gender;
  onGenderChange: (gender: Gender) => void;
  selected: CharacterVariant;
  onSelect: (variant: CharacterVariant) => void;
}

/** The preview tracks the equipped variant; browsing genders changes nothing until a variant is picked. */
function PersonPicker(props: PersonPickerProps) {
  const { gender, onGenderChange, selected, onSelect } = props;
  const { t } = useTranslation();
  const variants = personVariantsOf(gender);

  return (
    <PickerCard label={t("drawer.character.person_label")} selected={selected}>
      <View style={styles.chips}>
        {GENDERS.map((g) => (
          <Chip
            key={g}
            label={t(`drawer.character.${g}_label`)}
            active={gender === g}
            onPress={() => onGenderChange(g)}
          />
        ))}
      </View>
      <View style={styles.rule} />
      <View style={styles.chips}>
        {variants.map((v, i) => (
          <Chip
            key={v.key}
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
  selected: CharacterVariant;
  onSelect: (variant: CharacterVariant) => void;
}

function PetPicker(props: PetPickerProps) {
  const { selected, onSelect } = props;
  const { t } = useTranslation();

  return (
    <PickerCard label={t("drawer.character.pet_label")} selected={selected}>
      <View style={styles.chips}>
        {PET_VARIANTS.map((v) => (
          <Chip
            key={v.key}
            label={t(`pets.${v.key}`)}
            active={v.number === selected.number}
            onPress={() => onSelect(v)}
          />
        ))}
      </View>
    </PickerCard>
  );
}

/** Picks take effect at once and the sheet stays open, for comparing variants. */
export function CharacterPicker() {
  const person = personVariant(
    useKernel((s) => s.settings[SETTING.person.key] ?? -1),
  );
  const pet = petVariant(useKernel((s) => s.settings[SETTING.pet.key] ?? -1));

  // Opens on the selection's gender, so a pick does not look reset on reopen.
  const [gender, setGender] = useState<Gender>(genderOf(person));

  return (
    <View style={styles.pickers}>
      <PersonPicker
        gender={gender}
        onGenderChange={setGender}
        selected={person}
        onSelect={(v) => kernel_setting(SETTING.person.key, v.number)}
      />
      <PetPicker
        selected={pet}
        onSelect={(v) => kernel_setting(SETTING.pet.key, v.number)}
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
  preview: {
    width: PREVIEW,
    height: PREVIEW,
    flexShrink: 0,
    borderRadius: 12,
    overflow: "hidden",
    backgroundColor: alpha(color.fill, 0.4),
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
