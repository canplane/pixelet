// @pxl/web/app/settings/settings_modal.tsx

import { useTranslation } from "react-i18next";

import type { BaseModalProps } from "@/app/types/ui.types.ts";
import { X } from "lucide-react";

import { Sheet, SheetClose, SheetContent, SheetDescription, SheetTitle } from "@/shared/ui/theme/base/sheet.tsx";
import { Separator } from "@/shared/ui/theme/base/separator.tsx";
import { LanguageSection } from "./sections/language_section.tsx";
import { LightingSection } from "./sections/lighting_section.tsx";
import { PostFxSection } from "./sections/post_fx_section.tsx";
import { QualitySection } from "./sections/quality_section.tsx";
import { DebugSection } from "./sections/debug_section.tsx";
import { DisclaimerSection } from "./sections/disclaimer_section.tsx";
import { AboutSection } from "./sections/about_section.tsx";
import { AttributionSection } from "./sections/attribution_section.tsx";

/** SettingsModalProps */
export type SettingsModalProps = {
  type: "settings";
};

/**
 * Settings modal — rendering-effect toggles first (always reachable, real
 * graphics options, split by category into ./sections/*), then dev-only
 * diagnostics (__IS_DEV__-gated), the language switch, the disclaimers
 * (DisclaimerSection), the required OSM/Copernicus data-source
 * attribution (AttributionSection), and the build identity (AboutSection)
 * (was previously collapsed behind a map-corner ⓘ in bottom_overlays.tsx —
 * see that file's own removed comment for why this is its real home now).
 */
export function SettingsModal(props: BaseModalProps & SettingsModalProps) {
  const { open, on_open_change } = props;
  const { t } = useTranslation();

  return (
    <Sheet open={open} onOpenChange={on_open_change}>
      <SheetContent
        side="left"
        // The panel owns no padding of its own: the header and the scroll
        // area below set their own, which is what lets the header's rule run
        // edge to edge while the content it caps stays inset. p-5 (20) is
        // that inset — the rows inside bleed 12px outward (-mx-3, so a hover
        // fill reaches past the text), which at 16 left them 4px off the
        // panel edge, and at the sheet's stock 24 plus the body's old px-4
        // put 40px between the edge and the text on a 360px panel.
        //
        // Full width on a phone (the sheet's own `left` variant), 360 from
        // sm up. It used to be w-[min(360px,100vw)], which on a 375pt screen
        // left a 15px strip of live map down one side — too narrow to see
        // anything through and too wide to read as an edge, so the panel
        // looked misaligned rather than layered.
        hideClose
        className="sm:w-[360px] p-0 gap-0 flex flex-col"
        overlayClassName="bg-transparent"
        overlayStyle={{ pointerEvents: "none" }}
      >
        {
          /* Sticky, and a real row rather than shadcn's stacked header: the
          title used to scroll away with the content, which on a phone meant
          the panel lost its own name as soon as you moved. shrink-0 because
          it is a flex sibling of the scroll area and would otherwise be
          compressed by it.
          Padded past the top safe-area inset — this is a full-height panel
          on a phone, so its first row sits under the notch without it. */
        }
        <div className="shrink-0 border-b border-border/60" style={{ paddingTop: "env(safe-area-inset-top, 0px)" }}>
          {
            /* The safe-area inset lives on the wrapper and the row is a flat
            h-14 inside it. Padding on the row itself was the first version
            and it could not be symmetric: the inset has to be added to the
            top and nowhere else, so top and bottom were 20 and 12 and the
            bar came out ~74 tall. A fixed height with items-center centres
            whatever the row holds, whatever the inset turns out to be. */
          }
          <div className="flex h-14 items-center gap-2 px-5">
            <SheetTitle className="flex-1 text-[17px] font-semibold">{t("settings.title")}</SheetTitle>
            <SheetDescription className="sr-only">{t("settings.description")}</SheetDescription>
            {
              /* The sheet's own corner X is suppressed (hideClose) in favour of
            this one. A button in the header reads as part of the panel
            instead of as something floating over its first line, and it can
            carry a real 42px target — the stock one is a bare 16px glyph.
            -mr-2.5 pulls the button's box back so the glyph inside it lines
            up with the panel's own 20px inset rather than sitting 20px in
            from it. */
            }
            <SheetClose
              className="-mr-2.5 flex size-[42px] items-center justify-center rounded-full text-muted-foreground transition-colors hover:bg-accent hover:text-foreground focus:outline-none focus-visible:ring-2 focus-visible:ring-ring cursor-pointer"
              aria-label={t("settings.close")}
            >
              <X className="size-5" />
            </SheetClose>
          </div>
        </div>

        {
          /* The scroll area, not the panel. overscroll-contain stops a swipe
          that runs past the end here from continuing into the map behind.
          pt-4 and the same 1rem at the bottom, so the list is inset equally
          top and bottom — it was 1.5rem below, which read as the last
          section having been given extra room for no reason. The home
          indicator is already handled by the safe-area inset added to it;
          padding past that was double-counting the same clearance. */
        }
        <div
          className="flex-1 overflow-y-auto overscroll-contain flex flex-col gap-4 px-5 pt-4"
          style={{ paddingBottom: "calc(env(safe-area-inset-bottom, 0px) + 1rem)" }}
        >
          {
            /* Rendering first, because it is the only part anyone opens this
            panel to change. Language moved out of the top slot it used to
            hold: i18n.ts resolves the initial language from the browser, so
            for almost everyone this switch is already correct and is
            something to find rather than something to answer. It sits with
            the other read-once sections at the bottom instead. */
          }
          {
            /* The LOW/MED/HIGH chips on the rows below had no legend at all:
            SettingCost's own doc comment explained them to whoever reads
            TypeScript, and to nobody else. One line here rather than a note
            per section — the three sections that carry chips are
            consecutive, and this sits above all of them. Phrased as an
            instruction, since "relative render cost" is only useful once it
            says which one to turn off. */
          }
          <p className="text-[11px] leading-snug text-muted-foreground/70">{t("settings.cost_legend")}</p>

          <LightingSection />
          <PostFxSection />
          <QualitySection />

          {__IS_DEV__ && (
            <>
              <Separator />
              <DebugSection />
            </>
          )}

          <Separator />
          <LanguageSection />

          {
            /* One Separator for the three below, not one each. They are the
            same kind of section lighting/post_fx/quality already are above —
            consecutive and undivided, just text instead of controls — so
            drawing a line between every pair here while drawing none up
            there would be the inconsistency, not this. */
          }
          <Separator />
          <DisclaimerSection />
          <AttributionSection />
          <AboutSection />
        </div>
      </SheetContent>
    </Sheet>
  );
}
