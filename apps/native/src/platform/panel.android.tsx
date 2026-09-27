// @pxl/native/platform/panel.android.tsx

import { useContext, useEffect, useRef } from "react";
import { BackHandler } from "react-native";
import { PanelPresence, type PanelProps } from "@/shared/ui/panel.tsx";
import { Panel as PagePanel } from "../../../web/src/platform/web/panel.tsx";

// Android draws the page's own (panel.tsx is an iPhone's): its system
// controls are Material's, which read as any Android app's rather than this
// one's.
//
// What Material's sheet did for itself and the page's cannot: the system's
// back closes an open sheet. Unanswered, back leaves the app with the sheet
// still up.
export function Panel(props: PanelProps) {
  const { open } = useContext(PanelPresence);
  const onClose = useRef(props.onClose);
  onClose.current = props.onClose;
  useEffect(() => {
    if (!open) return;
    const back = BackHandler.addEventListener("hardwareBackPress", () => {
      onClose.current();
      return true;
    });
    return () => back.remove();
  }, [open]);
  return <PagePanel {...props} />;
}
