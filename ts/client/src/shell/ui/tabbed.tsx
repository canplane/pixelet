// @pxl/client/shell/ui/tabbed.tsx

import { type ReactNode, useState } from "react";

import { PanelBar, PanelBody, PanelScroll } from "@/shell/ui/panel.tsx";
import { Segmented, type SegmentedProps } from "@/shell/ui/segmented.tsx";

/** A panel's body whose top row is its tabs, opening on the first, and whose scroller is the tab's own. */
export function Tabbed<T extends string | number>({
  tabs,
  onClose,
  children,
}: {
  tabs: SegmentedProps<T>["segments"];
  onClose: () => void;
  children: (tab: T) => ReactNode;
}) {
  const [tab, setTab] = useState<T>(tabs[0].value);
  return (
    <PanelBody
      bar={
        <PanelBar onClose={onClose}>
          <Segmented value={tab} onChange={setTab} segments={tabs} />
        </PanelBar>
      }
    >
      <PanelScroll key={tab}>{children(tab)}</PanelScroll>
    </PanelBody>
  );
}
