// @pxl/web/app/settings/sections/disclaimer_section.tsx

import { Trans, useTranslation } from "react-i18next";

import { SectionTitle } from "@/app/settings/setting_row.tsx";

/**
 * What to expect and what not to — the limits, not the credits.
 *
 * One of three sections that used to be one. The split is by who is
 * speaking: this is the app setting expectations, AttributionSection is
 * other people's terms reproduced because they require it, AboutSection is
 * whose the app is and which build is running. Collapsed together, the disclaimers read as legal text
 * — the thing you scroll past — which is exactly backwards for a coverage
 * boundary that otherwise gets reported as a bug.
 *
 * The test for what belongs here: would someone be surprised, or think they
 * had hit a bug, without it. Everything below passes that.
 *
 * The texture line carries its own link rather than leaving a second credit
 * in the attribution section. CC0 obliges nothing, so one statement is enough,
 * and this is the one a reader actually reaches.
 */
export function DisclaimerSection() {
  const { t } = useTranslation();
  return (
    <div className="flex flex-col gap-1">
      <SectionTitle>{t("settings.disclaimer.section_title")}</SectionTitle>
      <div className="flex flex-col gap-2 py-1 text-[12px] text-muted-foreground">
        {
          /* First, ahead of every specific notice below it: this is a demo,
            not a product with a durability promise, and that framing has to
            land before anything it explains — the coverage/storage/location
            lines are instances of it, and reading them without this first
            makes each one sound like its own separate risk instead of one
            underlying fact stated once. */
        }
        <span>{t("settings.disclaimer.demo_notice")}</span>

        {
          /* Only Korean pages are baked (see the bake-tiles skill), and
            everywhere else falls back to the shared ocean tile — which looks
            like a bug rather than a coverage boundary unless it is written
            down somewhere. */
        }
        <span>{t("settings.disclaimer.coverage_notice")}</span>

        {
          /* Placed blocks live in localStorage (objects/store.ts) and go
            nowhere else. Worth saying plainly: this is a world-building app,
            so "will my work still be here" is the first thing anyone wonders,
            and the honest answer is "on this device, yes; anywhere else,
            no". */
        }
        <span>{t("settings.disclaimer.storage_notice")}</span>

        {
          /* Names Nominatim, drops the ~11m rounding the request applies (see
            use_reverse_geocode.ts). The service is worth naming — it is a
            third party, and "OSM Nominatim" means something to anyone who
            would care — while the rounding is a number nobody can act on. */
        }
        <span>{t("settings.disclaimer.location_notice")}</span>

        {
          /* Trans, not t(), because the link sits mid-sentence and both
            languages want it in a different place. Splitting the string
            around the anchor would leave two half-sentences for a translator
            to guess at; the <pack> tag keeps it one string. */
        }
        <span>
          <Trans
            i18nKey="settings.disclaimer.texture_notice"
            components={{
              pack: (
                <a
                  href="https://content.luanti.org/packages/StarNinjas/simplepixel/"
                  target="_blank"
                  rel="noreferrer"
                  className="underline hover:text-foreground"
                />
              ),
            }}
          />
        </span>
      </div>
    </div>
  );
}
