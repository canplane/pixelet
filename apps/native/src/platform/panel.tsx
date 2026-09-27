// @pxl/native/platform/panel.tsx

/**
 * A panel on an iPhone: the system's bottom sheet. Full height, or half
 * height (draggable to full) when `half`. The system draws the background,
 * drag handle and animation, dims the map, and closes on swipe down or a tap
 * on the map.
 *
 * A dismissal calls onClose right away; PanelPresence's `gone` is called
 * after the dismiss animation. Corner radii are the system's: the web's are
 * concentric with the top row's controls (shared/ui/panel.tsx SHEET_RADIUS),
 * but iOS cannot be set to that without leaving its standard.
 */

import { useContext } from "react";
import { StyleSheet, View } from "react-native";
import { BottomSheet, Group, Host, RNHostView } from "@expo/ui/swift-ui";
import { createModifier, ignoreSafeArea, presentationBackground, presentationDetents, presentationDragIndicator } from "@expo/ui/swift-ui/modifiers";

import { PanelPresence, type PanelProps } from "@/shared/ui/panel.tsx";
import { color } from "@/shared/ui/theme.ts";

/** SwiftUI's sheet. */
export function Panel({ label, onClose, half, children }: PanelProps) {
  const { open, gone } = useContext(PanelPresence);
  return (
    <Host style={styles.host}>
      <BottomSheet isPresented={open} onIsPresentedChange={(presented) => presented || onClose()} onDismiss={gone}>
        <Group
          modifiers={[
            presentationDetents(half ? ["medium", "large"] : ["large"]),
            presentationDragIndicator("visible"),
            presentationBackground(color.paper),
            // Dragging the content scrolls it; only the handle and header
            // resize or dismiss the sheet (modules/platform/ios/SheetContent.swift).
            createModifier("pxlContentScrolls"),
            // Content scrolls down to the sheet's bottom edge, under the home
            // indicator; each panel's scroll content pads its own end for it.
            ignoreSafeArea({ regions: "container", edges: "bottom" }),
          ]}
        >
          {
            /* The sheet is a separate view controller, so touches from the
            app's root never reach its content; RNHostView delivers them. */
          }
          <RNHostView>
            {
              /* Absolutely positioned to the host's edges. In normal flow the
              content's height became the host's height, so a long list was
              laid out at full length and its ScrollView could not scroll. */
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
