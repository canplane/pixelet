// @pxl/native/platform/panel.android.tsx

import { useContext, useEffect, useRef } from "react";
import { BackHandler } from "react-native";
import { PanelPresence, type PanelProps } from "@/shared/ui/panel.tsx";
import { Panel as PagePanel } from "../../../web/src/platform/web/panel.tsx";

// Android draws the page's sheet; the system back closes it, which Material's sheet did for itself.
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
