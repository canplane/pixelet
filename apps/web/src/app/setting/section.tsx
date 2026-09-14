// @pxl/web/app/setting/section.tsx

import type { LucideIcon } from "lucide-react";
import { Bug, Eye, Gauge, Move3d, Pickaxe, Sun } from "lucide-react";
import type { ReactNode } from "react";
import { useTranslation } from "react-i18next";

import { type SettingKey, SETTINGS, settings_in, type SettingSection, type ToggleKey } from "./schema.ts";
import { SectionTitle, SettingRadioRow, SettingRow } from "./setting_row.tsx";
import { kernel_action, useKernel } from "@/wasm/kernel.ts";

const SECTION_ICONS: Partial<Record<SettingSection, LucideIcon>> = {
  display: Eye,
  camera: Move3d,
  editing: Pickaxe,
  lighting: Sun,
  quality: Gauge,
  debug: Bug,
};

/** One section of the sheet, its rows generated from the schema. A row is
 * a switch for a toggle and a bordered radio group for a choice — one
 * answer, not one switch per option. Strings come from
 * `settings.<section>.<key>` and, for a choice, `.<option>` under that. */
export function SettingsSection(props: { section: SettingSection; children?: ReactNode }) {
  const { section, children } = props;
  const { t } = useTranslation();
  const keys = settings_in(section).filter((key) => __IS_DEV__ || !SETTINGS[key].dev);

  return (
    <div className="flex flex-col gap-1">
      <SectionTitle icon={SECTION_ICONS[section]} caption={t(`settings.${section}.section_caption`)} dev={section === "debug"}>
        {t(`settings.${section}.section_title`)}
      </SectionTitle>
      {keys.map((key) => <SettingControl key={key} name={key} />)}
      {children}
    </div>
  );
}

function SettingControl(props: { name: SettingKey }) {
  const { name } = props;
  const { t } = useTranslation();
  const setting = SETTINGS[name];
  const value = useKernel((s) => s.settings[name]);
  const set_setting = (key: string, option: string) => kernel_action({ type: "set_setting", key, value: option });
  const toggle_setting = (key: string) => kernel_action({ type: "toggle_setting", key });
  const base = `settings.${setting.section}.${name}`;

  switch (setting.kind) {
    case "toggle":
      return (
        <SettingRow
          label={t(`${base}.label`)}
          description={t(`${base}.description`)}
          active={value as boolean}
          on_toggle={() => toggle_setting(name as ToggleKey)}
          cost={setting.cost}
          dev={setting.dev && setting.section !== "debug"}
        />
      );
    case "choice":
      return (
        <div className="flex flex-col gap-0.5 -mx-3 rounded-[10px] border border-border p-1">
          {setting.options.map((option) => (
            <SettingRadioRow
              key={option}
              label={t(`${base}.${option}.label`)}
              description={t(`${base}.${option}.description`)}
              selected={value === option}
              on_select={() => set_setting(name, option)}
              cost={setting.cost}
            />
          ))}
        </div>
      );
    case "text":
      return null;
  }
}
