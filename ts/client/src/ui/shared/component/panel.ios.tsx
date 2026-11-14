// @pxl/client/ui/shared/component/panel.ios.tsx

import { useContext } from "react";
import { StyleSheet, View } from "react-native";
import { BottomSheet, Group, Host, RNHostView } from "@expo/ui/swift-ui";
import {
  createModifier,
  ignoreSafeArea,
  presentationBackground,
  presentationDetents,
  presentationDragIndicator,
} from "@expo/ui/swift-ui/modifiers";

import {
  PanelPresence,
  type PanelProps,
} from "@/ui/shared/component/panel.tsx";
import { COLOR } from "@/ui/shared/component/theme.ts";

// Every export of the default file, except `Panel`: an export declared in this
// file takes the place of a star export with the same name.
export * from "./panel.tsx";

export function Panel({ label, onClose, half, children }: PanelProps) {
  const { open, on_exited } = useContext(PanelPresence);
  return (
    <Host style={styles.host}>
      <BottomSheet
        isPresented={open}
        onIsPresentedChange={(presented) => presented || onClose()}
        onDismiss={on_exited}
      >
        <Group
          modifiers={[
            presentationDetents(half ? ["medium", "large"] : ["large"]),
            presentationDragIndicator("visible"),
            presentationBackground(COLOR.ground),
            createModifier("pxlContentScrolls"),
            ignoreSafeArea({ regions: "container", edges: "bottom" }),
          ]}
        >
          <RNHostView>
            {/* The content is laid out absolutely. In the flow, a long list
                takes its full height, and its ScrollView cannot scroll. */}
            <View accessibilityLabel={label} style={StyleSheet.absoluteFill}>
              {children}
            </View>
          </RNHostView>
        </Group>
      </BottomSheet>
    </Host>
  );
}

const styles = StyleSheet.create({
  host: { position: "absolute" },
});
