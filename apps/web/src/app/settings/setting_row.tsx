// @pxl/web/app/settings/setting_row.tsx

import type { LucideIcon } from "lucide-react";
import { useTranslation } from "react-i18next";

import { cn } from "@/shared/lib/utils/cn.ts";
import { Switch } from "@/shared/ui/theme/base/switch.tsx";

/** Relative render cost tier — a quick visual cue for which toggles are worth flipping on a weaker device, without repeating a cost sentence in every row's own description. */
export type SettingCost = "low" | "med" | "high";

const COST_CLASS: Record<SettingCost, string> = {
  low: "border-emerald-500/30 bg-emerald-500/10 text-emerald-600 dark:text-emerald-400",
  med: "border-amber-500/30 bg-amber-500/10 text-amber-600 dark:text-amber-400",
  high: "border-red-500/30 bg-red-500/10 text-red-600 dark:text-red-400",
};

function CostBadge(props: { cost: SettingCost }) {
  const { t } = useTranslation();
  return (
    <span
      className={cn(
        // py-0.5, up from py-px. leading-none means the line box is the font
        // size and nothing more, so 1px of vertical padding put the border
        // almost on the glyphs — the chip read as a rule around the text
        // rather than a pill containing it. The horizontal padding was
        // already fine at px-1; only the vertical axis was starved.
        "inline-flex items-center rounded-full border px-1 py-0.5 text-[9px] font-semibold leading-none tracking-wide",
        COST_CLASS[props.cost],
      )}
    >
      {t(`cost.${props.cost}`)}
    </span>
  );
}

type SettingRowProps = {
  label: string;
  description: string;
  active: boolean;
  on_toggle: () => void;
  /** Omit for toggles with no meaningful render cost (e.g. dev gizmos). */
  cost?: SettingCost;
};

/** One labeled toggle: name + cost chip + short description on the left, a switch on the right. */
export function SettingRow(props: SettingRowProps) {
  const { label, description, active, on_toggle, cost } = props;
  return (
    <label className="flex items-center justify-between gap-3 rounded-[8px] px-3 py-2.5 -mx-3 transition-colors hover:bg-accent/50 cursor-pointer">
      {/* gap-1, the same value SectionTitle uses between its own title and
      caption: a label and the line explaining it are the same relationship
      at both levels, and this pair had no gap at all. */}
      <div className="min-w-0 flex flex-col gap-1">
        <div className="flex items-center gap-1.5">
          <span className="text-[15px] font-medium leading-tight">{label}</span>
          {cost && <CostBadge cost={cost} />}
        </div>
        <div className="text-[12px] leading-snug text-muted-foreground">{description}</div>
      </div>
      <Switch checked={active} onCheckedChange={on_toggle} />
    </label>
  );
}

type SectionTitleProps = {
  children: string;
  /** Optional. The sections that hold controls carry one; the three at the
   * bottom that are only text (disclaimer, licence, about) deliberately do
   * not — an icon beside a heading you cannot act on reads as a button that
   * does nothing, and dropping them is what separates the two halves of the
   * panel without adding a rule. */
  icon?: LucideIcon;
  caption?: string;
};

/** Section heading — icon + label, plus an optional one-line caption shared across every row below it (so each row doesn't have to repeat the same cost/behavior note individually). */
export function SectionTitle(props: SectionTitleProps) {
  const { children, icon: Icon, caption } = props;
  return (
    <div className="flex flex-col gap-1 mb-1">
      <h3 className="flex items-center gap-1.5 text-[12px] font-semibold uppercase tracking-wide text-muted-foreground">
        {Icon && <Icon className="size-4" />}
        {children}
      </h3>
      {caption && <p className="text-[11px] text-muted-foreground/70">{caption}</p>}
    </div>
  );
}
