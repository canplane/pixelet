// @pxl/web/feature/setting/modal.tsx

import { useTranslation } from "react-i18next";

import type { BaseModalProps } from "@/app/type/ui.type.ts";
import { Info, Sparkles, X } from "lucide-react";

import { Sheet, SheetClose, SheetContent, SheetDescription, SheetTitle } from "@/shared/ui/shadcn/sheet.tsx";
import { Separator } from "@/shared/ui/shadcn/separator.tsx";
import { Tabs, TabsContent, TabsList, TabsTrigger } from "@/shared/ui/shadcn/tabs.tsx";
import { SettingsSection } from "./section.tsx";
import { LanguageSection } from "./section/language.tsx";
import { DisclaimerSection } from "./section/disclaimer.tsx";
import { AboutSection } from "./section/about.tsx";
import { AttributionSection } from "./section/attribution.tsx";

export interface SettingsModalProps {
  type: "settings";
}

// Each tab scrolls on its own. One scroller around both panels kept its
// scrollTop across a switch, so a long scroll down the effects list opened
// the info tab already scrolled past its top; Radix mounts a panel fresh
// when it becomes active, so a scroller per panel starts at zero every time.
const TAB_SCROLL = "flex-1 min-h-0 overflow-y-auto overscroll-contain px-5 pt-4";
const TAB_PAD = { paddingBottom: "calc(env(safe-area-inset-bottom, 0px) + 1rem)" } as const;

export function SettingsModal(props: BaseModalProps & SettingsModalProps) {
  const { open, onOpenChange } = props;
  const { t } = useTranslation();

  return (
    <Sheet open={open} onOpenChange={onOpenChange}>
      <SheetContent
        side="left"
        hideClose
        className="sm:w-[360px] p-0 gap-0 flex flex-col bg-background text-foreground"
        overlayStyle={{ pointerEvents: "none" }}
      >
        <div className="shrink-0 border-b border-border/60" style={{ paddingTop: "env(safe-area-inset-top, 0px)" }}>
          <div className="flex h-14 items-center gap-2 px-5">
            <SheetTitle className="flex-1 text-[17px] font-semibold">{t("settings.title")}</SheetTitle>
            <SheetDescription className="sr-only">{t("settings.description")}</SheetDescription>
            <SheetClose
              className="-mr-2.5 flex size-[42px] items-center justify-center rounded-full text-muted-foreground transition-colors hover:bg-accent hover:text-foreground focus:outline-none focus-visible:ring-2 focus-visible:ring-ring cursor-pointer"
              aria-label={t("settings.close")}
            >
              <X className="size-5" />
            </SheetClose>
          </div>
        </div>

        <Tabs defaultValue="effects" className="flex-1 min-h-0">
          <TabsList className="mx-5 mt-4">
            <TabsTrigger value="effects">
              <Sparkles className="size-3.5" />
              {t("settings.tabs.effects")}
            </TabsTrigger>
            <TabsTrigger value="info">
              <Info className="size-3.5" />
              {t("settings.tabs.info")}
            </TabsTrigger>
          </TabsList>

          <TabsContent value="effects" className={TAB_SCROLL} style={TAB_PAD}>
            <p className="text-[11px] leading-snug text-muted-foreground/70">{t("settings.cost_legend")}</p>
            <Separator />
            <SettingsSection section="display" />
            <SettingsSection section="camera" />
            <SettingsSection section="editing" />
            <SettingsSection section="lighting" />
            <SettingsSection section="quality" />
            <SettingsSection section="debug" />
          </TabsContent>

          <TabsContent value="info" className={TAB_SCROLL} style={TAB_PAD}>
            <LanguageSection />
            <Separator />
            <DisclaimerSection />
            <AttributionSection />
            <AboutSection />
          </TabsContent>
        </Tabs>
      </SheetContent>
    </Sheet>
  );
}
