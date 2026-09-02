// @pxl/web/app/settings/setting_row.tsx

import type { LucideIcon } from "lucide-react";

import { cn } from "@/shared/lib/utils/cn.ts";
import { Switch } from "@/shared/ui/shadcn/switch.tsx";

export type SettingCost = "low" | "med" | "high";

const COST_CLASS: Record<SettingCost, string> = {
  low: "border-emerald-500/30 bg-emerald-500/10 text-emerald-600",
  med: "border-amber-500/30 bg-amber-500/10 text-amber-600",
  high: "border-red-500/30 bg-red-500/10 text-red-600",
};

function CostBadge(props: { cost: SettingCost }) {
  return (
    <span
      className={cn(
        "inline-flex items-center rounded-full border px-1 py-0.5 text-[9px] font-semibold leading-none tracking-wide",
        COST_CLASS[props.cost],
      )}
    >
      {props.cost.toUpperCase()}
    </span>
  );
}

function DevBadge() {
  return (
    <span className="inline-flex items-center rounded-full border border-violet-500/30 bg-violet-500/10 px-1 py-0.5 text-[9px] font-semibold leading-none tracking-wide text-violet-600">
      DEV
    </span>
  );
}

type SettingRowProps = {
  label: string;
  description: string;
  active: boolean;
  on_toggle: () => void;
  cost?: SettingCost;
  /** Marks a setting only reachable in dev builds (rendered under `__IS_DEV__`) — callers gate visibility themselves, this just labels the row once shown. */
  dev?: boolean;
};

export function SettingRow(props: SettingRowProps) {
  const { label, description, active, on_toggle, cost, dev } = props;
  return (
    <label className="flex items-center justify-between gap-3 rounded-[8px] px-3 py-2.5 -mx-3 transition-colors hover:bg-accent/50 cursor-pointer">
      <div className="min-w-0 flex flex-col gap-1">
        <div className="flex items-center gap-1.5">
          <span className="text-[15px] font-medium leading-tight">{label}</span>
          {dev && <DevBadge />}
          {cost && <CostBadge cost={cost} />}
        </div>
        <div className="text-[12px] leading-snug text-muted-foreground">{description}</div>
      </div>
      <Switch checked={active} onCheckedChange={on_toggle} />
    </label>
  );
}

type SettingRadioRowProps = {
  label: string;
  description: string;
  selected: boolean;
  on_select: () => void;
  cost?: SettingCost;
};

/** One option in a mutually-exclusive settings group — same label/
 * description/cost-badge layout as SettingRow, a radio instead of a Switch
 * since selecting one option here deselects the others (see e.g.
 * settings_store.ts's aa_mode). No -mx-3 bleed like SettingRow: this is
 * meant to sit inside a small background-grouped wrapper (its own padding,
 * not the tab content's px-5), not directly against the tab content edge. */
export function SettingRadioRow(props: SettingRadioRowProps) {
  const { label, description, selected, on_select, cost } = props;
  return (
    <label className="flex items-center justify-between gap-3 rounded-[8px] px-3 py-2.5 transition-colors hover:bg-accent/50 cursor-pointer">
      <div className="min-w-0 flex flex-col gap-1">
        <div className="flex items-center gap-1.5">
          <span className="text-[15px] font-medium leading-tight">{label}</span>
          {cost && <CostBadge cost={cost} />}
        </div>
        <div className="text-[12px] leading-snug text-muted-foreground">{description}</div>
      </div>
      <input type="radio" checked={selected} onChange={on_select} className="size-4 shrink-0 accent-primary cursor-pointer" />
    </label>
  );
}

type SectionTitleProps = {
  children: string;
  icon?: LucideIcon;
  caption?: string;
  /** Marks the whole section as dev-only — callers still gate visibility
   * themselves (`__IS_DEV__ && <Section/>`), this just labels it once shown. */
  dev?: boolean;
};

export function SectionTitle(props: SectionTitleProps) {
  const { children, icon: Icon, caption, dev } = props;
  return (
    <div className="flex flex-col gap-1 mb-1">
      <h3 className="flex items-center gap-1.5 text-[12px] font-semibold uppercase tracking-wide text-muted-foreground">
        {Icon && <Icon className="size-4" />}
        {children}
        {dev && <DevBadge />}
      </h3>
      {caption && <p className="text-[11px] text-muted-foreground/70">{caption}</p>}
    </div>
  );
}
