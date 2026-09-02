// @pxl/web/app/settings/sections/character_section.tsx

import { useState } from "react";
import { Users } from "lucide-react";
import { useTranslation } from "react-i18next";

import { cn } from "@/shared/lib/utils/cn.ts";
import { SectionTitle } from "@/app/settings/setting_row.tsx";
import { CharacterPreview } from "@/modules/world/characters/character_preview.tsx";
import { useSettingsStore } from "@/app/settings/settings_store.ts";
import { find_person_variant, PERSON_VARIANTS_FEMALE, PERSON_VARIANTS_MALE, PET_VARIANTS } from "@pxl/shared/assets/characters/variants.ts";

// Every chip in this section — gender, person variant, pet variant alike —
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
      <span className="text-[13px] font-medium text-muted-foreground">{t("settings.character.person_label")}</span>
      <div className="flex gap-3 items-center">
        <CharacterPreview variant={selected} className="size-20 shrink-0 rounded-[10px] bg-accent/40" />
        <div className="flex flex-col gap-1.5 min-w-0">
          <div className="flex flex-wrap gap-1.5">
            {(["female", "male"] as const).map((g) => (
              <button key={g} type="button" onClick={() => on_gender_change(g)} className={chip_class(gender === g)}>
                {t(`settings.character.${g}_label`)}
              </button>
            ))}
          </div>
          <div className="h-px bg-border" />
          <div className="flex flex-wrap gap-1.5">
            {variants.map((v) => (
              <button key={v.key} type="button" onClick={() => on_select(v.key)} className={chip_class(v.key === selected_key)}>
                {t(`settings.character.persons.${v.key}`)}
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
      <span className="text-[13px] font-medium text-muted-foreground">{t("settings.character.pet_label")}</span>
      <div className="flex gap-3 items-center">
        <CharacterPreview variant={selected} className="size-20 shrink-0 rounded-[10px] bg-accent/40" />
        <div className="flex flex-wrap gap-1.5">
          {PET_VARIANTS.map((v) => (
            <button key={v.key} type="button" onClick={() => on_select(v.key)} className={chip_class(v.key === selected_key)}>
              {t(`settings.character.pets.${v.key}`)}
            </button>
          ))}
        </div>
      </div>
    </div>
  );
}

export function CharacterSection() {
  const { t } = useTranslation();

  const person_variant_key = useSettingsStore((state) => state.person_variant_key);
  const set_person_variant_key = useSettingsStore((state) => state.set_person_variant_key);
  const pet_variant_key = useSettingsStore((state) => state.pet_variant_key);
  const set_pet_variant_key = useSettingsStore((state) => state.set_pet_variant_key);

  // Opens on whichever gender the current selection already belongs to,
  // not always "female" — so a male variant picked earlier doesn't look
  // like it silently reset when the settings modal reopens.
  const [gender, set_gender] = useState<Gender>(
    PERSON_VARIANTS_MALE.some((v) => v.key === person_variant_key) ? "male" : "female",
  );

  return (
    <div className="flex flex-col gap-3">
      <SectionTitle icon={Users} caption={t("settings.character.section_caption")}>{t("settings.character.section_title")}</SectionTitle>
      <PersonPicker gender={gender} on_gender_change={set_gender} selected_key={person_variant_key} on_select={set_person_variant_key} />
      <PetPicker selected_key={pet_variant_key} on_select={set_pet_variant_key} />
    </div>
  );
}
