// @pxl/client/ui/shared/component/tabbed.tsx

import { type ReactNode, useState } from "react";

import {
  PanelBar,
  PanelBody,
  PanelScroll,
} from "@/ui/shared/component/panel.tsx";
import {
  Segmented,
  type SegmentedProps,
} from "@/ui/shared/component/segmented.tsx";

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
