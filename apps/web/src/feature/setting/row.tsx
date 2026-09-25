// @pxl/web/feature/setting/row.tsx

import type { LucideIcon } from "lucide-react";

import type { ReactNode } from "react";

import { cn } from "@/shared/lib/util/cn.ts";
import { Switch } from "@/shared/ui/shadcn/switch.tsx";
import type { SettingCost } from "@/feature/setting/schema.ts";

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

interface SettingRowProps {
  label: string;
  description: string;
  active: boolean;
  onToggle: () => void;
  cost?: SettingCost;
  /** Marks a setting only a development build offers (schema.json's `dev`) — the table decides whether the row is there at all, this just labels it once shown. */
  dev?: boolean;
}

export function SettingRow(props: SettingRowProps) {
  const { label, description, active, onToggle, cost, dev } = props;
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
      <Switch checked={active} onCheckedChange={onToggle} />
    </label>
  );
}

interface SettingGroupProps {
  label: string;
  cost?: SettingCost;
  dev?: boolean;
  children: ReactNode;
}

/** A choice's answers in one box, with the setting's name let into the line
 * that draws it — a fieldset's legend, drawn rather than declared. The
 * answers each name an answer and nothing said what they were answers to; a
 * name floating above the box would have been a second thing to place, so
 * the name sits on the border itself, carrying the sheet's own background
 * so the line appears to run through it on either side. The box takes the
 * top padding the name needs to clear the first answer, and the name is
 * inset to the left rather than centered, since every other line in the
 * sheet starts at the left — by its own padding, so the name starts exactly
 * where the answers' names start. A pixel above the line rather than
 * centered on it: the badges beside the name make the span taller than the
 * name is, and centering the span left the name reading low.
 *
 * The box stands away from the rows it sits among — a switch's row has only
 * the section's own gap around it, which left this one reading as another
 * row rather than as a box of answers. More above than below, because the
 * name rises out of the box by half its height and would otherwise close on
 * whatever is over it. */
export function SettingGroup(props: SettingGroupProps) {
  const { label, cost, dev, children } = props;
  return (
    <div className="relative -mx-3 mb-2.5 mt-4 rounded-[10px] border border-border p-1 pt-2.5">
      <span className="absolute -top-px left-2 flex -translate-y-1/2 items-center gap-1.5 bg-background px-2 text-[12px] font-medium leading-none">
        {label}
        {dev && <DevBadge />}
        {cost && <CostBadge cost={cost} />}
      </span>
      <div className="flex flex-col gap-0.5">{children}</div>
    </div>
  );
}

interface SettingRadioRowProps {
  label: string;
  description: string;
  selected: boolean;
  onSelect: () => void;
  cost?: SettingCost;
}

/** One option in a mutually-exclusive settings group — same label/
 * description/cost-badge layout as SettingRow, a radio instead of a Switch
 * since selecting one option here deselects the others (see e.g.
 * schema.ts's camera_terrain). No -mx-3 bleed like SettingRow: this is
 * meant to sit inside a small background-grouped wrapper (its own padding,
 * not the tab content's px-5), not directly against the tab content edge. */
export function SettingRadioRow(props: SettingRadioRowProps) {
  const { label, description, selected, onSelect, cost } = props;
  return (
    <label className="flex items-center justify-between gap-3 rounded-[8px] px-3 py-2.5 transition-colors hover:bg-accent/50 cursor-pointer">
      <div className="min-w-0 flex flex-col gap-1">
        <div className="flex items-center gap-1.5">
          <span className="text-[15px] font-medium leading-tight">{label}</span>
          {cost && <CostBadge cost={cost} />}
        </div>
        <div className="text-[12px] leading-snug text-muted-foreground">{description}</div>
      </div>
      <input type="radio" checked={selected} onChange={onSelect} className="size-4 shrink-0 accent-primary cursor-pointer" />
    </label>
  );
}

interface SectionTitleProps {
  children: string;
  icon?: LucideIcon;
  caption?: string;
  /** Marks the whole section as a development build's (schema.json's
   * DEV_SECTIONS) — section.tsx decides whether it is shown at all, this just
   * labels it once shown. */
  dev?: boolean;
}

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
