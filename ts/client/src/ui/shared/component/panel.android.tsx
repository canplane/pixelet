// @pxl/client/ui/shared/component/panel.android.tsx

import { useContext, useEffect, useRef } from "react";
import { BackHandler } from "react-native";
import {
  PanelPresence,
  type PanelProps,
} from "@/ui/shared/component/panel.tsx";
import { Panel as Sheet } from "./panel.tsx";

// Every export of the default file, except `Panel`: an export declared in this
// file takes the place of a star export with the same name.
export * from "./panel.tsx";

export function Panel(props: PanelProps) {
  const { open } = useContext(PanelPresence);
  const on_close = useRef(props.onClose);
  on_close.current = props.onClose;
  useEffect(() => {
    if (!open) return;
    const back = BackHandler.addEventListener("hardwareBackPress", () => {
      on_close.current();
      return true;
    });
    return () => back.remove();
  }, [open]);
  return <Sheet {...props} />;
}
