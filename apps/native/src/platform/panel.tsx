// @pxl/native/platform/panel.tsx

/**
 * A panel on an iPhone: the system's bottom sheet. `gone` is called after
 * the dismiss animation. Corner radii are the system's.
 */

import { useContext } from "react";
import { StyleSheet, View } from "react-native";
import { BottomSheet, Group, Host, RNHostView } from "@expo/ui/swift-ui";
import { createModifier, ignoreSafeArea, presentationBackground, presentationDetents, presentationDragIndicator } from "@expo/ui/swift-ui/modifiers";

import { PanelPresence, type PanelProps } from "@/shared/ui/panel.tsx";
import { color } from "@/shared/ui/theme.ts";

export function Panel({ label, onClose, half, children }: PanelProps) {
  const { open, gone } = useContext(PanelPresence);
  return (
    <Host style={styles.host}>
      <BottomSheet isPresented={open} onIsPresentedChange={(presented) => presented || onClose()} onDismiss={gone}>
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
            {
              /* Absolute: in flow, a long list took its full height and its
                            ScrollView could not scroll. */
            }
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
