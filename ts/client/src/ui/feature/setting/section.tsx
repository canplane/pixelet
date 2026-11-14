// @pxl/client/ui/feature/setting/section.tsx

import { useTranslation } from "react-i18next";
import type { GlyphName } from "@/ui/shared/component/icon.tsx";

import {
  SETTING,
  type Setting,
  type SettingName,
  type SettingSection,
  settings_in,
} from "@/ui/shared/setting.ts";
import { Section, SettingChoiceRow, SettingGroup, SettingRow } from "./row.tsx";
import { useCore } from "@/ui/shared/watch.ts";
import { set as setSetting } from "@/service/core/setting.ts";

const SECTION_ICONS: Partial<Record<SettingSection, GlyphName>> = {
  display: "eye",
  camera: "move.3d",
  lighting: "sun.max",
  debug: "ladybug",
};

/** Whether `option` is shown only in development. The table may hold no
 * setting with dev options, and then its type has no `dev_options`, so the
 * field is read this way. */
function is_dev_option(setting: Setting, option: string): boolean {
  const options =
    "dev_options" in setting ? (setting.dev_options as readonly string[]) : [];
  return options.includes(option);
}

export function SettingsSection(props: { section: SettingSection }) {
  const { section } = props;
  const { t } = useTranslation();
  const keys = settings_in(section);
  if (keys.length === 0) return null;

  return (
    <Section
      title={t(`settings.${section}.section_title`)}
      icon={SECTION_ICONS[section]}
      caption={t(`settings.${section}.section_caption`)}
    >
      {keys.map((key) => (
        <SettingControl key={key} name={key} />
      ))}
    </Section>
  );
}

function SettingControl(props: { name: SettingName }) {
  const { name } = props;
  const { t, i18n } = useTranslation();
  const setting: Setting = SETTING[name];
  const { key } = setting;
  const cost = "cost" in setting ? setting.cost : undefined;
  const dev = "dev" in setting && setting.dev;
  const value = useCore((s) => s.settings[key]);
  const base = `settings.${setting.section}.${name}`;

  switch (setting.kind) {
    case "toggle":
      return (
        <SettingRow
          label={t(`${base}.label`)}
          description={t(`${base}.description`)}
          active={value === 1}
          onChange={(on) => setSetting(key, on ? 1 : 0)}
          cost={cost}
          dev={dev}
        />
      );
    case "choice":
      return (
        <SettingGroup
          label={t(`${base}.label`)}
          description={
            i18n.exists(`${base}.description`)
              ? t(`${base}.description`)
              : undefined
          }
          cost={cost}
          dev={dev}
        >
          {Object.entries(setting.options).map(([option, number], i) => {
            const dev_only = is_dev_option(setting, option);
            return (
              (__DEV__ || !dev_only) && (
                <SettingChoiceRow
                  key={option}
                  label={t(`${base}.${option}.label`)}
                  description={t(`${base}.${option}.description`)}
                  selected={value === number}
                  onSelect={() => setSetting(key, number)}
                  cost={("costs" in setting ? setting.costs[i] : "") || cost}
                  dev={dev_only}
                />
              )
            );
          })}
        </SettingGroup>
      );
    case "index":
      return null;
  }
}
