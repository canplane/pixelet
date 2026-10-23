// @pxl/client/feature/character/picker.tsx

import { type ReactNode, useState } from "react";
import { useTranslation } from "react-i18next";
import { Pressable, StyleSheet, Text, View } from "react-native";

import { alpha, chrome, color, hairline, textScale, textStyle, weight } from "@/shared/ui/theme.ts";
import { SectionTitle } from "@/shared/ui/section-title.tsx";
import { CharacterPreview } from "@/feature/character/preview.tsx";
import { kernel_setting, useKernel } from "@/kernel/shell.ts";
import { SETTINGS } from "@/feature/setting/schema.ts";
import { type CharacterVariant, type Gender, genderOf, GENDERS, personVariant, personVariantsOf, PET_VARIANTS, petVariant } from "@/feature/character/variant.ts";
import { LABEL } from "@platform/palette.ts";
import { PANEL_EDGE, PANEL_SECTION_GAP } from "@/shared/ui/panel.tsx";

// The chip one sees is 32; a press lands on 44 (doc/minutes/20260928-three-sizes-of-a-control.md).
const CHIP_HEIGHT = 32;
const PREVIEW = 96;
const CHIP_GAP_X = 8;
const CHIP_GAP_Y = chrome.target - CHIP_HEIGHT;
const CHIP_REACH = { paddingHorizontal: CHIP_GAP_X / 2, paddingVertical: CHIP_GAP_Y / 2 };

function Chip({ label, active, onPress }: { label: string; active: boolean; onPress: () => void }) {
  return (
    <Pressable onPress={onPress} accessibilityRole="radio" aria-checked={active} style={CHIP_REACH}>
      <View style={[styles.chip, active && styles.chipOn]}>
        {
          /* Sized to the label's heavier weight, so a pick never rewraps the row. */
        }
        <Text aria-hidden maxFontSizeMultiplier={textScale.chrome} style={[styles.chipText, styles.chipTextOn, styles.sizer]}>{label}</Text>
        <View style={styles.label}>
          <Text maxFontSizeMultiplier={textScale.chrome} style={[styles.chipText, styles.centered, active && styles.chipTextOn]}>{label}</Text>
        </View>
      </View>
    </Pressable>
  );
}

/** A preview of the one picked beside the chips that pick it. */
function PickerCard({ label, selected, children }: { label: string; selected: CharacterVariant; children: ReactNode }) {
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
        {GENDERS.map((g) => <Chip key={g} label={t(`drawer.character.${g}_label`)} active={gender === g} onPress={() => onGenderChange(g)} />)}
      </View>
      <View style={styles.rule} />
      <View style={styles.chips}>
        {variants.map((v, i) => <Chip key={v.key} label={`${i + 1}`} active={v.number === selected.number} onPress={() => onSelect(v)} />)}
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
        {PET_VARIANTS.map((v) => <Chip key={v.key} label={t(`pets.${v.key}`)} active={v.number === selected.number} onPress={() => onSelect(v)} />)}
      </View>
    </PickerCard>
  );
}

/** Picks take effect at once and the sheet stays open, for comparing variants. */
export function CharacterPicker() {
  const person = personVariant(useKernel((s) => s.settings[SETTINGS.person.key] ?? -1));
  const pet = petVariant(useKernel((s) => s.settings[SETTINGS.pet.key] ?? -1));

  // Opens on the selection's gender, so a pick does not look reset on reopen.
  const [gender, setGender] = useState<Gender>(genderOf(person));

  return (
    <View style={styles.pickers}>
      <PersonPicker gender={gender} onGenderChange={setGender} selected={person} onSelect={(v) => kernel_setting(SETTINGS.person.key, v.number)} />
      <PetPicker selected={pet} onSelect={(v) => kernel_setting(SETTINGS.pet.key, v.number)} />
    </View>
  );
}

const styles = StyleSheet.create({
  pickers: { gap: PANEL_SECTION_GAP },
  picker: { gap: 8 },
  pickerRow: { flexDirection: "row", alignItems: "flex-start", gap: PANEL_EDGE },
  preview: { width: PREVIEW, height: PREVIEW, flexShrink: 0, borderRadius: 12, overflow: "hidden", backgroundColor: alpha(color.fill, 0.4) },
  chipColumn: { flexShrink: 1, minWidth: 0, gap: CHIP_GAP_Y / 2, marginHorizontal: -CHIP_GAP_X / 2, marginVertical: -CHIP_GAP_Y / 2 },
  chips: { flexDirection: "row", flexWrap: "wrap" },
  rule: { height: 1, marginHorizontal: CHIP_GAP_X / 2, backgroundColor: hairline },
  chip: { height: CHIP_HEIGHT, justifyContent: "center", borderRadius: CHIP_HEIGHT / 2, paddingHorizontal: 12, backgroundColor: color.faint },
  chipOn: { backgroundColor: color.chosen },
  chipText: { ...textStyle.footnote, fontWeight: weight.medium, color: LABEL.primary },
  chipTextOn: { fontWeight: weight.semibold, color: color.ground },
  sizer: { opacity: 0 },
  label: { ...StyleSheet.absoluteFill, justifyContent: "center" },
  centered: { textAlign: "center" },
});
