// @pxl/client/ui/component/panel.android.tsx

import { useContext, useEffect, useRef } from "react";
import { BackHandler } from "react-native";
import { PanelPresence, type PanelProps } from "@/ui/component/panel.tsx";
import { Panel as Sheet } from "./panel.tsx";

// Every export of the default file, except `Panel`: an export declared in this
// file takes the place of a star export with the same name.
export * from "./panel.tsx";

// Android draws the page's sheet, and the system back button closes it.
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
  return <Sheet {...props} />;
}
