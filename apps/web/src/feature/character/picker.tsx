// @pxl/web/feature/character/picker.tsx

import { useState } from "react";
import { useTranslation } from "react-i18next";
import { Pressable, StyleSheet, Text, View } from "react-native";

import { alpha, color, hairline, sectionTitle, textStyle, weight } from "@/shared/ui/theme.ts";
import { CharacterPreview } from "@/feature/character/preview.tsx";
import type { u32 } from "@xpute/core/abi/word.ts";
import { Command, encode_setting_update } from "@/kernel/abi/cmd.spec.ts";
import { kernel_command, useKernel } from "@/kernel/shell.ts";
import { SETTINGS } from "@/feature/setting/schema.ts";
import { findPersonVariant, findPetVariant, type Gender, genderOf, GENDERS, personVariantNumbered, personVariantsOf, PET_VARIANTS } from "@/feature/character/variant.ts";
import { LABEL } from "@platform/palette.ts";
import { PANEL_EDGE, PANEL_SECTION_GAP } from "@/shared/ui/panel.tsx";

// Not a setting: person/pet variant is a companion *appearance* pick, the
// same "which of these do I want equipped" shape as a cube color or decor
// kind, just always-on-one-of-each instead of placed-many — so it lives in
// drawer-modal.tsx's Character tab rather than the settings sheet.

// Every chip in this component — gender, person variant, pet variant alike —
// is this one component, not a look-alike, since a gender chip and a variant
// chip are literally the same kind of control.
// Its three sizes (issue/20260928-three-sizes-of-a-control.md): a press lands
// on 44, the least target, reaching into the gaps around it by half, which
// the gaps are sized to hold so no two chips' targets meet; the chip one sees
// is 32, as a Material chip is, a choice set in a panel; its words are the
// footnote's.
const CHIP_HEIGHT = 32;
/** A companion's preview, a square. */
const PREVIEW = 96;
const CHIP_GAP_X = 8;
const CHIP_GAP_Y = 44 - CHIP_HEIGHT;
const CHIP_SLOP = { top: CHIP_GAP_Y / 2, bottom: CHIP_GAP_Y / 2, left: CHIP_GAP_X / 2, right: CHIP_GAP_X / 2 };

function Chip({ label, active, onPress }: { label: string; active: boolean; onPress: () => void }) {
  return (
    <Pressable onPress={onPress} hitSlop={CHIP_SLOP} accessibilityRole="radio" aria-checked={active} style={[styles.chip, active && styles.chipOn]}>
      {
        /* The chip is as wide as its label in the heavier weight, whichever it
      shows: a pick that widened its chip would rewrap the row under the
      finger. */
      }
      <Text aria-hidden style={[styles.chipText, styles.chipTextOn, styles.sizer]}>{label}</Text>
      <Text style={[styles.chipText, styles.label, active && styles.chipTextOn]}>{label}</Text>
    </Pressable>
  );
}

interface PersonPickerProps {
  gender: Gender;
  onGenderChange: (gender: Gender) => void;
  selectedKey: string;
  onSelect: (key: string) => void;
}

/** Preview always tracks the actually-equipped variant (findPersonVariant
 * searches both genders), never the currently-browsed gender's own first
 * entry — switching the gender row is just browsing a different chip set,
 * not a selection, so neither the preview nor the equipped companion may
 * change until a variant chip itself is clicked. */
function PersonPicker(props: PersonPickerProps) {
  const { gender, onGenderChange, selectedKey, onSelect } = props;
  const { t } = useTranslation();
  const selected = findPersonVariant(selectedKey);
  const variants = personVariantsOf(gender);

  return (
    <View style={styles.picker}>
      <Text style={styles.pickerLabel}>{t("drawer_modal.character.person_label")}</Text>
      <View style={styles.pickerRow}>
        <CharacterPreview variant={selected} style={styles.preview} />
        <View style={styles.chipColumn}>
          <View style={styles.chips}>
            {GENDERS.map((g) => <Chip key={g} label={t(`drawer_modal.character.${g}_label`)} active={gender === g} onPress={() => onGenderChange(g)} />)}
          </View>
          <View style={styles.rule} />
          <View style={styles.chips}>
            {variants.map((v) => <Chip key={v.key} label={t(`drawer_modal.character.persons.${v.key}`)} active={v.key === selectedKey} onPress={() => onSelect(v.key)} />)}
          </View>
        </View>
      </View>
    </View>
  );
}

interface PetPickerProps {
  selectedKey: string;
  onSelect: (key: string) => void;
}

function PetPicker(props: PetPickerProps) {
  const { selectedKey, onSelect } = props;
  const { t } = useTranslation();
  const selected = findPetVariant(selectedKey);

  return (
    <View style={styles.picker}>
      <Text style={styles.pickerLabel}>{t("drawer_modal.character.pet_label")}</Text>
      <View style={styles.pickerRow}>
        <CharacterPreview variant={selected} style={styles.preview} />
        <View style={[styles.chips, styles.chipColumn]}>
          {PET_VARIANTS.map((v) => <Chip key={v.key} label={t(`drawer_modal.character.pets.${v.key}`)} active={v.key === selectedKey} onPress={() => onSelect(v.key)} />)}
        </View>
      </View>
    </View>
  );
}

/** drawer-modal.tsx's Character tab content — equips the single always-on
 * companion (the one the kernel's layer/character/ draws), never a
 * placed-into-the-world pick the way Cube/Decor are; selecting a chip here
 * takes effect immediately and the sheet stays open (unlike CubeSwatchButton/
 * DecorGridButton, which close it) since browsing/comparing several variants
 * is the expected flow, not "equip once then go place it." */
export function CharacterPicker() {
  // The settings hold a variant as its number; the picker reads its key off
  // the same catalog the kernel was numbered from.
  const personVariant = useKernel((s) => s.settings[SETTINGS.person.key] ?? -1);
  const petVariant = useKernel((s) => s.settings[SETTINGS.pet.key] ?? -1);
  const personVariantKey = personVariantNumbered(personVariant)?.key ?? "";
  const petVariantKey = PET_VARIANTS.find((v) => v.number === petVariant)?.key ?? "";
  const setSetting = (key: u32, value: u32) => kernel_command(Command.SETTING_UPDATE, encode_setting_update({ key, value }));

  // Opens on whichever gender the current selection already belongs to,
  // not always the first — so a variant of another picked earlier doesn't
  // look like it silently reset when the drawer reopens.
  const [gender, setGender] = useState<Gender>(genderOf(personVariantKey));

  return (
    <View style={styles.pickers}>
      <PersonPicker gender={gender} onGenderChange={setGender} selectedKey={personVariantKey} onSelect={(key) => setSetting(SETTINGS.person.key, findPersonVariant(key).number)} />
      <PetPicker selectedKey={petVariantKey} onSelect={(key) => setSetting(SETTINGS.pet.key, findPetVariant(key).number)} />
    </View>
  );
}

const styles = StyleSheet.create({
  pickers: { gap: PANEL_SECTION_GAP },
  picker: { gap: 8 },
  pickerLabel: { ...sectionTitle, color: LABEL.secondary },
  // The preview stands off the chips by the sheet's own edge.
  pickerRow: { flexDirection: "row", alignItems: "flex-start", gap: PANEL_EDGE },
  preview: { width: PREVIEW, height: PREVIEW, flexShrink: 0, borderRadius: 12, overflow: "hidden", backgroundColor: alpha(color.fill, 0.4) },
  chipColumn: { flexShrink: 1, minWidth: 0, gap: CHIP_GAP_Y },
  chips: { flexDirection: "row", flexWrap: "wrap", columnGap: CHIP_GAP_X, rowGap: CHIP_GAP_Y },
  rule: { height: 1, backgroundColor: hairline },
  chip: { height: CHIP_HEIGHT, justifyContent: "center", borderRadius: CHIP_HEIGHT / 2, paddingHorizontal: 12, backgroundColor: color.faint },
  chipOn: { backgroundColor: color.chosen },
  chipText: { ...textStyle.footnote, fontWeight: weight.medium, color: LABEL.primary },
  chipTextOn: { fontWeight: weight.semibold, color: color.paper },
  sizer: { opacity: 0 },
  label: { position: "absolute", left: 0, right: 0, top: (CHIP_HEIGHT - textStyle.footnote.lineHeight) / 2, textAlign: "center" },
});
