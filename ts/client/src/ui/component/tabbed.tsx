// @pxl/client/ui/component/tabbed.tsx

import { type ReactNode, useState } from "react";

import { PanelBar, PanelBody, PanelScroll } from "@/ui/component/panel.tsx";
import { Segmented, type SegmentedProps } from "@/ui/component/segmented.tsx";

/** A panel's body whose top row holds its tabs. It opens on the first tab, and
 * each tab has a scroller of its own. */
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
