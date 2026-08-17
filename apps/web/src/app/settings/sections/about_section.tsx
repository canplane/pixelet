// @pxl/web/app/settings/sections/about_section.tsx

import { Box } from "lucide-react";
import { useTranslation } from "react-i18next";

import { SectionTitle } from "@/app/settings/setting_row.tsx";
import { APP_ENV, BUILD_TIME, HAS_STATIC_TILES, TILE_BASE_URL } from "@/lib/env/config.ts";

/** Last path segment of the tile base with any leading `v` dropped — `1` out
 * of `/tiles/v1`, or out of a full origin, so the label can name the version
 * without naming the host. A dated version (`v2026-08-16`) reads the same
 * way. */
const tile_version: string = (TILE_BASE_URL.split("/").filter(Boolean).pop() ?? "").replace(/^v/, "");

/** `development` -> `Development`. Sentence case because it sits at the head
 * of a sentence-shaped line, not because the value itself is capitalised. */
const env_label: string = APP_ENV.charAt(0).toUpperCase() + APP_ENV.slice(1);

/**
 * The build stamp as `YYYYMMDDHHmm` — a token, not a sentence. It sits
 * beside "Tile version 1" and answers the same kind of question, and the
 * way it gets used is comparison against the last value rather than reading
 * a date, which a formatted one is worse at.
 *
 * Local getters, so it reads in the viewer's own timezone. BUILD_TIME is
 * UTC (vite.config.ts) precisely so that can be decided here: formatting at
 * build time would bake in the builder's timezone, and "is this the deploy
 * that just went out?" is asked from wherever the reader is. The cost is
 * that two readers in different zones see different digits for one build —
 * acceptable while the only person comparing is the one who deployed it. If
 * that stops being true, print UTC and append a Z rather than dropping the
 * zone silently.
 */
function build_label(): string {
  const d = new Date(BUILD_TIME);
  if (Number.isNaN(d.getTime())) return BUILD_TIME;
  const pad = (n: number) => String(n).padStart(2, "0");
  return `${d.getFullYear()}${pad(d.getMonth() + 1)}${pad(d.getDate())}${pad(d.getHours())}${pad(d.getMinutes())}`;
}

/**
 * Which app this is and which build of it is running. Last of the three
 * sections that used to be one — see disclaimer_section.tsx for how they
 * divide.
 *
 * Last rather than first, and deliberately: a name and a version at the foot
 * of a settings list is where people already look for them, and neither is
 * something anyone opened this panel to read.
 */
export function AboutSection() {
  const { t } = useTranslation();
  return (
    <div className="flex flex-col gap-1">
      <SectionTitle>{t("settings.about.section_title")}</SectionTitle>
      <div className="flex flex-col gap-2 py-1 text-[12px] text-muted-foreground">
        {/* The cube stays. It is the app's mark rather than a section
            icon — the three headings below the controls dropped theirs (see
            SectionTitle), and this one is next to the name it belongs to. */}
        <div className="flex items-center gap-2 text-[15px] font-semibold text-foreground">
          <Box className="size-5" aria-hidden="true" />
          <span>pixelet</span>
        </div>

        {/* What this actually is, for whoever landed on a link with no other
            context. One line, directly under the name it describes — not a
            hero paragraph, since nobody opened Settings to read one. */}
        <span>{t("settings.about.tagline")}</span>

        {/* Moved out of the attribution list — see attribution_section.tsx
            for why a rights claim and a list of acknowledgements should not
            share a section, or a name. Here it sits under the app name it
            actually applies to. */}
        <span>
          © 2025{" "}
          <a href="https://github.com/canplane" target="_blank" rel="noreferrer" className="underline hover:text-foreground">
            canplane
          </a>
          . All rights reserved.
        </span>

        {/* Which build this is. Not a debug-only surface on purpose: the
            question it answers comes up precisely on a phone, where there is
            no URL bar or devtools to check.

            The environment name is omitted for production — that is the
            answer anyone reading a deployed build already assumes, so
            printing it is noise, while anything else is the part worth
            seeing. Compared against the string rather than gated on
            __IS_DEV__ so a build labelled `staging` still says so.

            It names the source rather than printing TILE_BASE_URL. A bare
            `/tiles/v1` or a full origin is an implementation detail wearing
            the clothes of information: nothing a reader can act on, and an
            infrastructure address in the UI. The version is the part that
            carries meaning.

            Untranslated, unlike everything in the disclaimer section. That
            is copy the app addresses to its user; this is a build
            identifier, the same category as the version number inside it,
            and it reads as one line of English in either language rather
            than as something missing a translation. */}
        {/* Build stamp before the tile source: it is the one part that
            changes on every deploy, and so the one anyone is actually
            reading. The tile version moves years at a time. In production
            the environment is omitted entirely, which would otherwise leave
            a constant sitting in front of the value being checked. */}
        <span className="text-white/30">
          {APP_ENV !== "production" && `${env_label} · `}
          {build_label()}
          {` · ${HAS_STATIC_TILES ? `Tile version ${tile_version}` : "Live tile"}`}
        </span>
      </div>
    </div>
  );
}
