// @pxl/web/app/settings/sections/character_section.tsx

import { Users } from "lucide-react";
import { useTranslation } from "react-i18next";

import { cn } from "@/shared/lib/utils/cn.ts";
import { SectionTitle } from "@/app/settings/setting_row.tsx";
import { CharacterPreview } from "@/modules/world/characters/character_preview.tsx";
import { useSettingsStore } from "@/app/settings/settings_store.ts";
import { type CharacterVariant, PERSON_VARIANTS, PET_VARIANTS } from "@pxl/shared/assets/characters/variants.ts";

type VariantPickerProps = {
  label: string;
  variants: readonly CharacterVariant[];
  selected_key: string;
  on_select: (key: string) => void;
};

function VariantPicker(props: VariantPickerProps) {
  const { label, variants, selected_key, on_select } = props;
  const selected = variants.find((v) => v.key === selected_key) ?? variants[0];

  return (
    <div className="flex flex-col gap-2">
      <span className="text-[13px] font-medium text-muted-foreground">{label}</span>
      <div className="flex gap-3 items-center">
        <CharacterPreview variant={selected} className="size-20 shrink-0 rounded-[10px] bg-accent/40" />
        <div className="flex flex-wrap gap-1.5">
          {variants.map((v) => (
            <button
              key={v.key}
              type="button"
              onClick={() => on_select(v.key)}
              className={cn(
                "rounded-full border px-2.5 py-1 text-[12px] font-medium leading-none transition-colors cursor-pointer",
                v.key === selected_key ? "border-primary bg-primary/10 text-primary" : "border-border/60 text-muted-foreground hover:bg-accent/50",
              )}
            >
              {v.label}
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

  return (
    <div className="flex flex-col gap-3">
      <SectionTitle icon={Users} caption={t("settings.character.section_caption")}>{t("settings.character.section_title")}</SectionTitle>
      <VariantPicker
        label={t("settings.character.person_label")}
        variants={PERSON_VARIANTS}
        selected_key={person_variant_key}
        on_select={set_person_variant_key}
      />
      <VariantPicker
        label={t("settings.character.pet_label")}
        variants={PET_VARIANTS}
        selected_key={pet_variant_key}
        on_select={set_pet_variant_key}
      />
    </div>
  );
}
