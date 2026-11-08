// @pxl/client/ui/component/panel.ios.tsx

/**
 * A panel on an iPhone, drawn as the system's bottom sheet with the system's
 * corner radii. The sheet calls `gone` once its dismiss animation has ended.
 */

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

import { PanelPresence, type PanelProps } from "@/ui/component/panel.tsx";
import { color } from "@/ui/component/theme.ts";

// Every export of the default file, except `Panel`: an export declared in this
// file takes the place of a star export with the same name.
export * from "./panel.tsx";

export function Panel({ label, onClose, half, children }: PanelProps) {
  const { open, gone } = useContext(PanelPresence);
  return (
    <Host style={styles.host}>
      <BottomSheet
        isPresented={open}
        onIsPresentedChange={(presented) => presented || onClose()}
        onDismiss={gone}
      >
        <Group
          modifiers={[
            presentationDetents(half ? ["medium", "large"] : ["large"]),
            presentationDragIndicator("visible"),
            presentationBackground(color.ground),
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
