// @pxl/web/feature/character/picker.tsx

import { useState } from "react";
import { useTranslation } from "react-i18next";

import { cn } from "@/shared/lib/util/cn.ts";
import { CharacterPreview } from "@/feature/character/preview.tsx";
import { kernel_action, useKernel } from "@/kernel/shell.ts";
import { find_person_variant, PERSON_VARIANTS_FEMALE, PERSON_VARIANTS_MALE, PET_VARIANTS } from "@/feature/character/variant.ts";

// Not a setting: person/pet variant is a companion *appearance* pick, the
// same "which of these do I want equipped" shape as a cube color or decor
// kind, just always-on-one-of-each instead of placed-many — so it lives in
// drawer-modal.tsx's Character tab rather than the settings sheet.

// Every chip in this component — gender, person variant, pet variant alike —
// shares this exact class, not just a visually-similar one, since the point
// of chip_class existing at all is that a gender chip and a variant chip
// are literally the same kind of control.
function chip_class(active: boolean): string {
  return cn(
    "rounded-full border px-2.5 py-1 text-[12px] font-medium leading-none transition-colors cursor-pointer",
    active ? "border-primary bg-primary/10 text-primary" : "border-border/60 text-muted-foreground hover:bg-accent/50",
  );
}

type Gender = "female" | "male";

type PersonPickerProps = {
  gender: Gender;
  on_gender_change: (gender: Gender) => void;
  selected_key: string;
  on_select: (key: string) => void;
};

/** Preview always tracks the actually-equipped variant (find_person_variant
 * searches both genders), never the currently-browsed gender's own first
 * entry — switching the gender row is just browsing a different chip set,
 * not a selection, so neither the preview nor the equipped companion may
 * change until a variant chip itself is clicked. */
function PersonPicker(props: PersonPickerProps) {
  const { gender, on_gender_change, selected_key, on_select } = props;
  const { t } = useTranslation();
  const selected = find_person_variant(selected_key);
  const variants = gender === "female" ? PERSON_VARIANTS_FEMALE : PERSON_VARIANTS_MALE;

  return (
    <div className="flex flex-col gap-2">
      <span className="text-[13px] font-medium text-muted-foreground">{t("drawer_modal.character.person_label")}</span>
      <div className="flex gap-3 items-center">
        <CharacterPreview variant={selected} className="size-20 shrink-0 rounded-[10px] bg-accent/40" />
        <div className="flex flex-col gap-1.5 min-w-0">
          <div className="flex flex-wrap gap-1.5">
            {(["female", "male"] as const).map((g) => (
              <button key={g} type="button" onClick={() => on_gender_change(g)} className={chip_class(gender === g)}>
                {t(`drawer_modal.character.${g}_label`)}
              </button>
            ))}
          </div>
          <div className="h-px bg-border" />
          <div className="flex flex-wrap gap-1.5">
            {variants.map((v) => (
              <button key={v.key} type="button" onClick={() => on_select(v.key)} className={chip_class(v.key === selected_key)}>
                {t(`drawer_modal.character.persons.${v.key}`)}
              </button>
            ))}
          </div>
        </div>
      </div>
    </div>
  );
}

type PetPickerProps = {
  selected_key: string;
  on_select: (key: string) => void;
};

function PetPicker(props: PetPickerProps) {
  const { selected_key, on_select } = props;
  const { t } = useTranslation();
  const selected = PET_VARIANTS.find((v) => v.key === selected_key) ?? PET_VARIANTS[0];

  return (
    <div className="flex flex-col gap-2">
      <span className="text-[13px] font-medium text-muted-foreground">{t("drawer_modal.character.pet_label")}</span>
      <div className="flex gap-3 items-center">
        <CharacterPreview variant={selected} className="size-20 shrink-0 rounded-[10px] bg-accent/40" />
        <div className="flex flex-wrap gap-1.5">
          {PET_VARIANTS.map((v) => (
            <button key={v.key} type="button" onClick={() => on_select(v.key)} className={chip_class(v.key === selected_key)}>
              {t(`drawer_modal.character.pets.${v.key}`)}
            </button>
          ))}
        </div>
      </div>
    </div>
  );
}

/** drawer-modal.tsx's Character tab content — equips the single always-on
 * companion (world.tsx's own one `<Companion>` instance), never a
 * placed-into-the-world pick the way Cube/Decor are; selecting a chip here
 * takes effect immediately and the sheet stays open (unlike CubeSwatchButton/
 * DecorGridButton, which close it) since browsing/comparing several variants
 * is the expected flow, not "equip once then go place it." */
export function CharacterPicker() {
  // The settings hold a variant as its number; the picker reads its key off
  // the same catalog the kernel was numbered from.
  const person_variant = useKernel((s) => Number(s.settings.person_variant ?? -1));
  const pet_variant = useKernel((s) => Number(s.settings.pet_variant ?? -1));
  const person_variant_key = [...PERSON_VARIANTS_FEMALE, ...PERSON_VARIANTS_MALE].find((v) => v.number === person_variant)?.key ?? "";
  const pet_variant_key = PET_VARIANTS.find((v) => v.number === pet_variant)?.key ?? "";
  const set_setting = (key: string, value: number) => kernel_action({ type: "set_setting", key, value });

  // Opens on whichever gender the current selection already belongs to,
  // not always "female" — so a male variant picked earlier doesn't look
  // like it silently reset when the drawer reopens.
  const [gender, set_gender] = useState<Gender>(
    PERSON_VARIANTS_MALE.some((v) => v.key === person_variant_key) ? "male" : "female",
  );

  return (
    <div className="flex flex-col gap-4">
      <PersonPicker gender={gender} on_gender_change={set_gender} selected_key={person_variant_key} on_select={(key) => set_setting("person_variant", find_person_variant(key).number)} />
      <PetPicker selected_key={pet_variant_key} on_select={(key) => set_setting("pet_variant", (PET_VARIANTS.find((v) => v.key === key) ?? PET_VARIANTS[0]).number)} />
    </div>
  );
}
